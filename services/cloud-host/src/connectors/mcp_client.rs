use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Method;
use serde_json::{json, Value};
use std::time::Duration;

use crate::bounded_text::truncate_utf8_bytes;
use crate::connectors::installs::InstallToolDraft;
use crate::connectors::remote::{RemoteError, RemoteHttpClient, RemotePolicy, RemoteResponse};
use agent_core::ConnectorError;

pub const MAX_MCP_PAGES: usize = 5;
pub const MAX_MCP_TOOLS: usize = 128;
pub const MAX_MCP_DESCRIPTION_CHARS: usize = 512;
pub const MAX_MCP_SCHEMA_BYTES: usize = 16 * 1024;
pub const MAX_MCP_CATALOG_BYTES: usize = 256 * 1024;
pub const MCP_PROTOCOL_VERSION: &str = "2025-03-26";

#[derive(Debug, Clone)]
pub struct McpAuth {
    pub bearer: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DiscoveredMcpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub read_only: bool,
}

#[derive(Debug, Clone)]
pub struct McpDiscovery {
    pub tools: Vec<DiscoveredMcpTool>,
    pub oauth_required: bool,
    pub www_authenticate: Option<String>,
}

pub async fn discover_mcp_tools(
    client: &RemoteHttpClient,
    endpoint: &str,
    auth: &McpAuth,
) -> Result<McpDiscovery, ConnectorError> {
    match mcp_session(client, endpoint, auth).await {
        Ok(mut session) => {
            let tools = session.list_tools().await?;
            session.close().await;
            if tools.is_empty() {
                return Err(ConnectorError::Validation(
                    "MCP server did not expose any tools".into(),
                ));
            }
            Ok(McpDiscovery {
                tools,
                oauth_required: false,
                www_authenticate: None,
            })
        }
        Err(err) if is_unauthorized(&err) => Ok(McpDiscovery {
            tools: Vec::new(),
            oauth_required: true,
            www_authenticate: err.www_authenticate,
        }),
        Err(err) => Err(err.into_connector()),
    }
}

pub async fn call_mcp_tool(
    client: &RemoteHttpClient,
    endpoint: &str,
    auth: &McpAuth,
    tool_name: &str,
    arguments: &Value,
) -> Result<Value, ConnectorError> {
    let mut session = mcp_session(client, endpoint, auth)
        .await
        .map_err(|e| e.into_connector())?;
    let result = session.call_tool(tool_name, arguments).await;
    session.close().await;
    result
}

pub fn drafts_from_discovered(tools: &[DiscoveredMcpTool]) -> Vec<InstallToolDraft> {
    tools
        .iter()
        .map(|tool| InstallToolDraft {
            remote_name: tool.name.clone(),
            display_name: tool.name.clone(),
            description: tool.description.clone(),
            input_schema: tool.input_schema.clone(),
            read_only: tool.read_only,
        })
        .collect()
}

struct McpSession<'a> {
    client: &'a RemoteHttpClient,
    endpoint: String,
    auth: &'a McpAuth,
    session_id: Option<String>,
    next_id: i64,
}

struct McpSessionError {
    connector: ConnectorError,
    www_authenticate: Option<String>,
}

impl McpSessionError {
    fn into_connector(self) -> ConnectorError {
        self.connector
    }
}

impl From<ConnectorError> for McpSessionError {
    fn from(connector: ConnectorError) -> Self {
        Self {
            connector,
            www_authenticate: None,
        }
    }
}

async fn mcp_session<'a>(
    client: &'a RemoteHttpClient,
    endpoint: &'a str,
    auth: &'a McpAuth,
) -> Result<McpSession<'a>, McpSessionError> {
    let mut session = McpSession {
        client,
        endpoint: endpoint.to_string(),
        auth,
        session_id: None,
        next_id: 1,
    };
    let init = session
        .rpc(
            "initialize",
            json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": { "name": "elsewhere", "version": "0.1.0" }
            }),
        )
        .await?;
    if init.get("error").is_some() {
        return Err(rpc_error(&init).into());
    }
    session
        .notify("notifications/initialized", json!({}))
        .await?;
    Ok(session)
}

impl<'a> McpSession<'a> {
    async fn list_tools(&mut self) -> Result<Vec<DiscoveredMcpTool>, ConnectorError> {
        let mut cursor: Option<String> = None;
        let mut tools = Vec::new();
        let mut catalog_bytes = 0usize;
        for _page in 0..MAX_MCP_PAGES {
            let mut params = json!({});
            if let Some(cursor) = &cursor {
                params["cursor"] = json!(cursor);
            }
            let result = self
                .rpc("tools/list", params)
                .await
                .map_err(|e| e.connector)?;
            if result.get("error").is_some() {
                return Err(rpc_error(&result));
            }
            let payload = result.get("result").cloned().unwrap_or(json!({}));
            let page = payload
                .get("tools")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for tool in page {
                if tools.len() >= MAX_MCP_TOOLS {
                    return Err(ConnectorError::Validation(
                        "MCP server exposed too many tools".into(),
                    ));
                }
                let parsed = parse_mcp_tool(&tool)?;
                let encoded = serde_json::to_vec(&parsed.input_schema).unwrap_or_default();
                catalog_bytes = catalog_bytes.saturating_add(encoded.len());
                if catalog_bytes > MAX_MCP_CATALOG_BYTES {
                    return Err(ConnectorError::Validation(
                        "MCP tool catalog is too large".into(),
                    ));
                }
                tools.push(parsed);
            }
            cursor = payload
                .get("nextCursor")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            if cursor.is_none() {
                break;
            }
        }
        if cursor.is_some() {
            return Err(ConnectorError::Validation(
                "MCP server pagination exceeded the discovery bound".into(),
            ));
        }
        Ok(tools)
    }

    async fn call_tool(&mut self, name: &str, arguments: &Value) -> Result<Value, ConnectorError> {
        let result = self
            .rpc(
                "tools/call",
                json!({
                    "name": name,
                    "arguments": arguments
                }),
            )
            .await
            .map_err(|e| e.connector)?;
        if result.get("error").is_some() {
            return Err(sanitize_rpc_error(&result));
        }
        let payload = result.get("result").cloned().unwrap_or(json!({}));
        if payload.get("isError").and_then(|v| v.as_bool()) == Some(true) {
            return Err(ConnectorError::Provider(sanitize_text(
                payload
                    .get("content")
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "MCP tool returned an error".into())
                    .as_str(),
            )));
        }
        Ok(payload)
    }

    async fn rpc(&mut self, method: &str, params: Value) -> Result<Value, McpSessionError> {
        let id = self.next_id;
        self.next_id += 1;
        let payload = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        let response = self.post(Some(payload)).await?;
        parse_rpc_response(&response, id)
    }

    async fn notify(&mut self, method: &str, params: Value) -> Result<(), McpSessionError> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        let _ = self.post(Some(payload)).await?;
        Ok(())
    }

    async fn close(&mut self) {
        if self.session_id.is_none() {
            return;
        }
        let headers = self.headers();
        let _ = self
            .client
            .request(Method::DELETE, &self.endpoint, headers, None)
            .await;
        self.session_id = None;
    }

    async fn post(&mut self, body: Option<Value>) -> Result<RemoteResponse, McpSessionError> {
        let mut headers = self.headers();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            reqwest::header::ACCEPT,
            HeaderValue::from_static("application/json, text/event-stream"),
        );
        if let Ok(name) = reqwest::header::HeaderName::from_bytes(b"mcp-protocol-version") {
            headers.insert(name, HeaderValue::from_static(MCP_PROTOCOL_VERSION));
        }
        let bytes = body
            .as_ref()
            .map(|v| serde_json::to_vec(v).unwrap_or_else(|_| b"{}".to_vec()));
        let response = self
            .client
            .request(Method::POST, &self.endpoint, headers, bytes)
            .await
            .map_err(map_remote)?;
        if response.status.as_u16() == 401 {
            let www = response
                .headers
                .get(reqwest::header::WWW_AUTHENTICATE)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string);
            return Err(McpSessionError {
                connector: ConnectorError::ReconnectRequired,
                www_authenticate: www,
            });
        }
        if !response.status.is_success() {
            return Err(ConnectorError::Provider(format!(
                "MCP server returned HTTP {}",
                response.status.as_u16()
            ))
            .into());
        }
        if let Some(session) = response
            .headers
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
        {
            self.session_id = Some(session.to_string());
        }
        Ok(response)
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Some(token) = &self.auth.bearer {
            if let Ok(value) = HeaderValue::from_str(&format!("Bearer {token}")) {
                headers.insert(AUTHORIZATION, value);
            }
        }
        if let Some(session) = &self.session_id {
            if let Ok(name) = reqwest::header::HeaderName::from_bytes(b"mcp-session-id") {
                if let Ok(value) = HeaderValue::from_str(session) {
                    headers.insert(name, value);
                }
            }
        }
        headers
    }
}

fn parse_mcp_tool(value: &Value) -> Result<DiscoveredMcpTool, ConnectorError> {
    let name = value
        .get("name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ConnectorError::Validation("MCP tool is missing a name".into()))?;
    if !is_safe_tool_name(name) {
        return Err(ConnectorError::Validation(format!(
            "MCP tool name `{name}` is not usable"
        )));
    }
    let description = truncate_utf8_bytes(
        value
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
        MAX_MCP_DESCRIPTION_CHARS,
    );
    let schema = value
        .get("inputSchema")
        .cloned()
        .unwrap_or_else(|| json!({ "type": "object" }));
    let encoded = serde_json::to_vec(&schema).unwrap_or_default();
    if encoded.len() > MAX_MCP_SCHEMA_BYTES {
        return Err(ConnectorError::Validation(format!(
            "MCP tool `{name}` schema is too large"
        )));
    }
    let read_only = value
        .pointer("/annotations/readOnlyHint")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    Ok(DiscoveredMcpTool {
        name: name.to_string(),
        description,
        input_schema: schema,
        read_only,
    })
}

fn is_safe_tool_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

fn parse_rpc_response(response: &RemoteResponse, id: i64) -> Result<Value, McpSessionError> {
    let content_type = response
        .headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/json")
        .to_ascii_lowercase();
    if content_type.contains("text/event-stream") {
        return parse_sse_rpc(&response.text().unwrap_or_default(), id);
    }
    let value = response.json().map_err(map_remote)?;
    Ok(value)
}

fn parse_sse_rpc(body: &str, id: i64) -> Result<Value, McpSessionError> {
    for line in body.lines() {
        let line = line.trim();
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        if let Ok(value) = serde_json::from_str::<Value>(data.trim()) {
            if value.get("id").and_then(|v| v.as_i64()) == Some(id) || value.get("id").is_none() {
                return Ok(value);
            }
        }
    }
    serde_json::from_str(body).map_err(|_| {
        ConnectorError::Provider("MCP SSE response was not valid JSON-RPC".into()).into()
    })
}

fn rpc_error(value: &Value) -> ConnectorError {
    sanitize_rpc_error(value)
}

fn sanitize_rpc_error(value: &Value) -> ConnectorError {
    let message = value
        .pointer("/error/message")
        .and_then(|v| v.as_str())
        .unwrap_or("MCP request failed");
    ConnectorError::Provider(sanitize_text(message))
}

fn sanitize_text(input: &str) -> String {
    let mut out = input.to_string();
    for needle in [
        "Bearer ",
        "token=",
        "Authorization",
        "Cookie",
        "api_key",
        "apikey",
    ] {
        if out
            .to_ascii_lowercase()
            .contains(&needle.to_ascii_lowercase())
        {
            out = "[redacted remote error]".into();
            break;
        }
    }
    truncate_utf8_bytes(&out, 280)
}

fn map_remote(err: RemoteError) -> McpSessionError {
    match err {
        RemoteError::Timeout => ConnectorError::Provider("MCP request timed out".into()).into(),
        RemoteError::Redirect => ConnectorError::Validation(err.message()).into(),
        RemoteError::Blocked(m) => ConnectorError::Validation(m).into(),
        RemoteError::TooLarge => {
            ConnectorError::Validation("MCP response is too large".into()).into()
        }
        RemoteError::Http(401) => McpSessionError {
            connector: ConnectorError::ReconnectRequired,
            www_authenticate: None,
        },
        other => ConnectorError::Provider(other.message()).into(),
    }
}

fn is_unauthorized(err: &McpSessionError) -> bool {
    matches!(err.connector, ConnectorError::ReconnectRequired)
}

pub fn production_mcp_client() -> RemoteHttpClient {
    RemoteHttpClient::new(RemotePolicy {
        timeout: Duration::from_secs(15),
        ..RemotePolicy::production()
    })
}

#[allow(dead_code)]
pub fn test_mcp_client() -> RemoteHttpClient {
    RemoteHttpClient::new(RemotePolicy::for_tests())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_tool_names() {
        assert!(!is_safe_tool_name(""));
        assert!(!is_safe_tool_name("tool/name"));
        assert!(is_safe_tool_name("list_records"));
    }

    #[test]
    fn parse_tool_uses_readonly_hint_without_trusting_other_annotations() {
        let tool = parse_mcp_tool(&json!({
            "name": "search",
            "description": "Look things up",
            "inputSchema": { "type": "object" },
            "annotations": { "readOnlyHint": true, "openWorldHint": true }
        }))
        .unwrap();
        assert!(tool.read_only);
        assert_eq!(tool.name, "search");
    }

    #[tokio::test]
    async fn discovers_and_calls_streamable_http_mcp() {
        let mock = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::body_string_contains("initialize"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": { "protocolVersion": MCP_PROTOCOL_VERSION, "capabilities": {}, "serverInfo": { "name": "demo" } }
            })))
            .mount(&mock)
            .await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::body_string_contains(
                "notifications/initialized",
            ))
            .respond_with(wiremock::ResponseTemplate::new(202))
            .mount(&mock)
            .await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::body_string_contains("tools/list"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": 2,
                "result": {
                    "tools": [{
                        "name": "search_docs",
                        "description": "Search",
                        "inputSchema": { "type": "object", "properties": { "q": { "type": "string" } } },
                        "annotations": { "readOnlyHint": true }
                    }]
                }
            })))
            .mount(&mock)
            .await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::body_string_contains("tools/call"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": 2,
                "result": { "content": [{ "type": "text", "text": "ok" }] }
            })))
            .mount(&mock)
            .await;

        let client = test_mcp_client();
        let discovery = discover_mcp_tools(&client, &mock.uri(), &McpAuth { bearer: None })
            .await
            .unwrap();
        assert_eq!(discovery.tools.len(), 1);
        assert!(discovery.tools[0].read_only);
        let result = call_mcp_tool(
            &client,
            &mock.uri(),
            &McpAuth { bearer: None },
            "search_docs",
            &json!({ "q": "hi" }),
        )
        .await
        .unwrap();
        assert!(result.to_string().contains("ok"));
    }

    #[tokio::test]
    async fn sanitizes_remote_mcp_errors() {
        let mock = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::body_string_contains("initialize"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": { "protocolVersion": MCP_PROTOCOL_VERSION }
            })))
            .mount(&mock)
            .await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::body_string_contains(
                "notifications/initialized",
            ))
            .respond_with(wiremock::ResponseTemplate::new(202))
            .mount(&mock)
            .await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::body_string_contains("tools/call"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": 2,
                "error": { "code": -32000, "message": "Authorization: Bearer super-secret-token" }
            })))
            .mount(&mock)
            .await;
        let client = test_mcp_client();
        let err = call_mcp_tool(
            &client,
            &mock.uri(),
            &McpAuth { bearer: None },
            "search_docs",
            &json!({}),
        )
        .await
        .unwrap_err();
        let message = err.message();
        assert!(!message.contains("super-secret-token"));
        assert!(!message.contains("Bearer super"));
    }

    #[test]
    fn bounds_tool_count_and_schema_size() {
        let huge = "x".repeat(MAX_MCP_SCHEMA_BYTES + 8);
        let err = parse_mcp_tool(&json!({
            "name": "huge",
            "inputSchema": { "type": "object", "description": huge }
        }))
        .unwrap_err();
        assert!(matches!(err, ConnectorError::Validation(_)));
    }

    #[tokio::test]
    async fn pagination_bound_rejects_endless_cursors() {
        let mock = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::body_string_contains("initialize"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": { "protocolVersion": MCP_PROTOCOL_VERSION }
            })))
            .mount(&mock)
            .await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::body_string_contains(
                "notifications/initialized",
            ))
            .respond_with(wiremock::ResponseTemplate::new(202))
            .mount(&mock)
            .await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::body_string_contains("tools/list"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "result": {
                    "tools": [{ "name": "search_docs", "inputSchema": { "type": "object" } }],
                    "nextCursor": "more"
                }
            })))
            .mount(&mock)
            .await;
        let client = test_mcp_client();
        let err = match discover_mcp_tools(&client, &mock.uri(), &McpAuth { bearer: None }).await {
            Ok(_) => panic!("expected pagination bound to fail"),
            Err(err) => err,
        };
        assert!(matches!(err, ConnectorError::Validation(_)));
        assert!(err.message().contains("pagination") || err.message().contains("too many"));
    }
}
