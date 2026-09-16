use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Method;
use serde_json::{json, Map, Value};
use url::Url;

use crate::bounded_text::truncate_utf8_bytes;
use crate::connectors::installs::{InstallToolDraft, StoredSecret};
use crate::connectors::remote::{RemoteError, RemoteHttpClient, RemoteResponse};
use agent_core::ConnectorError;

pub const MAX_OPENAPI_OPS: usize = 128;
pub const MAX_OPENAPI_DESCRIPTION_CHARS: usize = 512;
const FORBIDDEN_HEADER_NAMES: &[&str] = &[
    "authorization",
    "cookie",
    "set-cookie",
    "host",
    "connection",
    "proxy-authorization",
    "proxy-authenticate",
    "proxy-connection",
    "forwarded",
    "x-forwarded-for",
    "x-forwarded-host",
    "x-forwarded-proto",
    "x-real-ip",
    "transfer-encoding",
    "te",
    "trailer",
    "upgrade",
    "keep-alive",
    "content-length",
];

#[derive(Debug, Clone)]
pub struct OpenApiImport {
    pub base_url: String,
    pub tools: Vec<InstallToolDraft>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct OperationSpec {
    remote_name: String,
    method: Method,
    path: String,
    description: String,
    read_only: bool,
    path_params: Vec<String>,
    query_params: Vec<String>,
    has_body: bool,
    input_schema: Value,
}

pub async fn import_openapi(
    client: &RemoteHttpClient,
    document_url: &str,
) -> Result<OpenApiImport, ConnectorError> {
    let response = client
        .request(Method::GET, document_url, HeaderMap::new(), None)
        .await
        .map_err(map_remote)?;
    if !response.status.is_success() {
        return Err(ConnectorError::Validation(format!(
            "OpenAPI document returned HTTP {}",
            response.status.as_u16()
        )));
    }
    let text = response
        .text()
        .map_err(|_| ConnectorError::Validation("OpenAPI document is not UTF-8".into()))?;
    if looks_like_yaml(&text) {
        return Err(ConnectorError::Validation(
            "OpenAPI YAML is not supported in v1; provide a JSON document".into(),
        ));
    }
    let doc: Value = serde_json::from_str(&text)
        .map_err(|_| ConnectorError::Validation("OpenAPI document is not valid JSON".into()))?;
    parse_openapi_document(&doc, document_url, client).await
}

async fn parse_openapi_document(
    doc: &Value,
    document_url: &str,
    client: &RemoteHttpClient,
) -> Result<OpenApiImport, ConnectorError> {
    let server = doc
        .get("servers")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.get("url"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            ConnectorError::Validation("OpenAPI document is missing a server URL".into())
        })?;
    let base_url = resolve_server_url(document_url, server)?;
    client.validate_url(&base_url).await.map_err(map_remote)?;

    let paths = doc
        .get("paths")
        .and_then(|v| v.as_object())
        .ok_or_else(|| ConnectorError::Validation("OpenAPI document has no paths".into()))?;
    let mut tools = Vec::new();
    let mut used_names = std::collections::HashSet::new();
    for (path, item) in paths {
        let Some(item) = item.as_object() else {
            continue;
        };
        for (method_name, spec) in item {
            let Some(method) = parse_method(method_name) else {
                continue;
            };
            let Some(op) = spec.as_object() else {
                continue;
            };
            let mut operation = operation_from_spec(path, method.clone(), op, &mut used_names)?;
            operation.input_schema = with_routing(operation.input_schema, method.as_str(), path);
            tools.push(operation);
            if tools.len() > MAX_OPENAPI_OPS {
                return Err(ConnectorError::Validation(
                    "OpenAPI document has too many operations".into(),
                ));
            }
        }
    }
    if tools.is_empty() {
        return Err(ConnectorError::Validation(
            "OpenAPI document did not contain any usable operations".into(),
        ));
    }
    Ok(OpenApiImport {
        base_url,
        tools: tools
            .into_iter()
            .map(|op| {
                let remote_name = op.remote_name;
                InstallToolDraft {
                    display_name: remote_name.clone(),
                    remote_name,
                    description: op.description,
                    input_schema: op.input_schema,
                    read_only: op.read_only,
                }
            })
            .collect(),
    })
}

fn operation_from_spec(
    path: &str,
    method: Method,
    op: &Map<String, Value>,
    used_names: &mut std::collections::HashSet<String>,
) -> Result<OperationSpec, ConnectorError> {
    let remote_name = unique_operation_name(
        op.get("operationId").and_then(|v| v.as_str()),
        method.as_str(),
        path,
        used_names,
    );
    let description = truncate_utf8_bytes(
        op.get("summary")
            .or_else(|| op.get("description"))
            .and_then(|v| v.as_str())
            .unwrap_or(""),
        MAX_OPENAPI_DESCRIPTION_CHARS,
    );
    let mut path_params = Vec::new();
    let mut query_params = Vec::new();
    let mut properties = Map::new();
    let mut required = Vec::new();
    if let Some(params) = op.get("parameters").and_then(|v| v.as_array()) {
        for param in params {
            let name = param.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if name.is_empty() || !is_safe_param_name(name) {
                continue;
            }
            let location = param.get("in").and_then(|v| v.as_str()).unwrap_or("");
            let schema = param
                .get("schema")
                .cloned()
                .unwrap_or_else(|| json!({ "type": "string" }));
            match location {
                "path" => path_params.push(name.to_string()),
                "query" => query_params.push(name.to_string()),
                _ => continue,
            }
            properties.insert(name.to_string(), schema);
            if param.get("required").and_then(|v| v.as_bool()) == Some(true) || location == "path" {
                required.push(Value::String(name.to_string()));
            }
        }
    }
    let has_body = matches!(method, Method::POST | Method::PUT | Method::PATCH)
        && op.get("requestBody").is_some();
    if has_body {
        let op_value = Value::Object(op.clone());
        let body_schema = op_value
            .pointer("/requestBody/content/application/json/schema")
            .cloned()
            .unwrap_or_else(|| json!({ "type": "object" }));
        properties.insert("body".into(), body_schema);
        if op_value
            .pointer("/requestBody/required")
            .and_then(|v| v.as_bool())
            == Some(true)
        {
            required.push(Value::String("body".into()));
        }
    }
    let read_only = method == Method::GET;
    let input_schema = json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    });
    Ok(OperationSpec {
        remote_name,
        method,
        path: path.to_string(),
        description,
        read_only,
        path_params,
        query_params,
        has_body,
        input_schema,
    })
}

pub async fn execute_openapi_operation(
    client: &RemoteHttpClient,
    base_url: &str,
    remote_name: &str,
    input_schema: &Value,
    arguments: &Value,
    secret: Option<&StoredSecret>,
) -> Result<Value, ConnectorError> {
    let _ = remote_name;
    reject_model_headers(arguments)?;
    if arguments.get("url").is_some()
        || arguments.get("endpoint").is_some()
        || arguments.get("method").is_some()
        || arguments.get("path").is_some()
    {
        return Err(ConnectorError::Validation(
            "OpenAPI execute does not accept endpoint or method overrides".into(),
        ));
    }
    let method = input_schema
        .get("x-elsewhereMethod")
        .and_then(|v| v.as_str())
        .and_then(parse_method)
        .ok_or_else(|| ConnectorError::Validation("OpenAPI operation is not executable".into()))?;
    let path_template = input_schema
        .get("x-elsewherePath")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ConnectorError::Validation("OpenAPI operation is not executable".into()))?
        .to_string();

    let mut path = path_template.clone();
    if let Some(props) = input_schema.get("properties").and_then(|v| v.as_object()) {
        for key in props.keys() {
            if key == "body" {
                continue;
            }
            if path.contains(&format!("{{{key}}}")) {
                let value = arguments
                    .get(key)
                    .and_then(|v| {
                        v.as_str().map(str::to_string).or_else(|| {
                            if v.is_number() || v.is_boolean() {
                                Some(v.to_string())
                            } else {
                                None
                            }
                        })
                    })
                    .ok_or_else(|| {
                        ConnectorError::Validation(format!("missing path parameter `{key}`"))
                    })?;
                path = path.replace(&format!("{{{key}}}"), &urlencoding::encode(&value));
            }
        }
    }
    if path.contains('{') {
        return Err(ConnectorError::Validation(
            "OpenAPI path still contains unsubstituted parameters".into(),
        ));
    }

    let mut url = join_url(base_url, &path)?;
    if let Some(props) = input_schema.get("properties").and_then(|v| v.as_object()) {
        for key in props.keys() {
            if key == "body" || path_template.contains(&format!("{{{key}}}")) {
                continue;
            }
            if let Some(value) = arguments.get(key) {
                let as_str = match value {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    _ => continue,
                };
                url.query_pairs_mut().append_pair(key, &as_str);
            }
        }
    }

    client
        .validate_url(url.as_str())
        .await
        .map_err(map_remote)?;

    let mut headers = HeaderMap::new();
    apply_secret_headers(&mut headers, secret)?;

    let body = if method != Method::GET {
        arguments.get("body").cloned()
    } else {
        None
    };
    if body.is_some() {
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    }
    let bytes = body
        .as_ref()
        .map(|v| serde_json::to_vec(v).unwrap_or_else(|_| b"{}".to_vec()));
    let response = client
        .request(method, url.as_str(), headers, bytes)
        .await
        .map_err(map_remote)?;
    response_to_value(response)
}

fn apply_secret_headers(
    headers: &mut HeaderMap,
    secret: Option<&StoredSecret>,
) -> Result<(), ConnectorError> {
    let Some(secret) = secret else {
        return Ok(());
    };
    match secret.kind.as_str() {
        "bearer" | "oauth" => {
            let token = secret
                .access_token
                .as_deref()
                .or(secret.token.as_deref())
                .ok_or(ConnectorError::ReconnectRequired)?;
            let value = HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|_| ConnectorError::Internal("invalid bearer token".into()))?;
            headers.insert(AUTHORIZATION, value);
        }
        "api_key_header" => {
            let name = secret.header_name.as_deref().ok_or_else(|| {
                ConnectorError::Validation("API key header is not configured".into())
            })?;
            if is_forbidden_header(name) {
                return Err(ConnectorError::Validation(
                    "configured API key header is not allowed".into(),
                ));
            }
            let token = secret
                .token
                .as_deref()
                .ok_or_else(|| ConnectorError::Validation("API key is missing".into()))?;
            let header_name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| ConnectorError::Validation("invalid API key header name".into()))?;
            let value = HeaderValue::from_str(token)
                .map_err(|_| ConnectorError::Internal("invalid API key".into()))?;
            headers.insert(header_name, value);
        }
        _ => {}
    }
    Ok(())
}

fn reject_model_headers(arguments: &Value) -> Result<(), ConnectorError> {
    if let Some(headers) = arguments.get("headers") {
        if let Some(obj) = headers.as_object() {
            for key in obj.keys() {
                if is_forbidden_header(key) {
                    return Err(ConnectorError::Validation(format!(
                        "header `{key}` cannot be set by the model"
                    )));
                }
            }
        }
        return Err(ConnectorError::Validation(
            "model-controlled headers are not allowed".into(),
        ));
    }
    Ok(())
}

pub fn is_forbidden_header(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    FORBIDDEN_HEADER_NAMES.contains(&lower.as_str()) || lower.starts_with("proxy-")
}

fn unique_operation_name(
    operation_id: Option<&str>,
    method: &str,
    path: &str,
    used: &mut std::collections::HashSet<String>,
) -> String {
    let candidate = operation_id
        .map(sanitize_operation_id)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| generated_operation_id(method, path));
    let mut name = candidate.clone();
    let mut i = 2;
    while used.contains(&name) {
        name = format!("{candidate}_{i}");
        i += 1;
    }
    used.insert(name.clone());
    name
}

fn sanitize_operation_id(raw: &str) -> String {
    let mut out = String::new();
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    out.chars().take(80).collect()
}

fn generated_operation_id(method: &str, path: &str) -> String {
    let mut slug = String::new();
    for c in path.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.ends_with('_') {
            slug.push('_');
        }
    }
    format!("{}_{}", method.to_ascii_lowercase(), slug.trim_matches('_'))
}

fn parse_method(name: &str) -> Option<Method> {
    match name.to_ascii_lowercase().as_str() {
        "get" => Some(Method::GET),
        "post" => Some(Method::POST),
        "put" => Some(Method::PUT),
        "patch" => Some(Method::PATCH),
        "delete" => Some(Method::DELETE),
        _ => None,
    }
}

fn resolve_server_url(document_url: &str, server: &str) -> Result<String, ConnectorError> {
    if let Ok(url) = Url::parse(server) {
        return Ok(url.to_string());
    }
    let doc = Url::parse(document_url)
        .map_err(|_| ConnectorError::Validation("OpenAPI document URL is invalid".into()))?;
    doc.join(server)
        .map(|u| u.to_string())
        .map_err(|_| ConnectorError::Validation("OpenAPI server URL is invalid".into()))
}

fn join_url(base: &str, path: &str) -> Result<Url, ConnectorError> {
    let base = Url::parse(base)
        .map_err(|_| ConnectorError::Validation("OpenAPI base URL is invalid".into()))?;
    let path = path.trim_start_matches('/');
    base.join(path)
        .map_err(|_| ConnectorError::Validation("OpenAPI request URL is invalid".into()))
}

fn looks_like_yaml(text: &str) -> bool {
    let trimmed = text.trim_start();
    !trimmed.starts_with('{') && (trimmed.starts_with("openapi:") || trimmed.starts_with("---"))
}

fn response_to_value(response: RemoteResponse) -> Result<Value, ConnectorError> {
    if let Ok(value) = response.json() {
        return Ok(json!({
            "ok": response.status.is_success(),
            "status": response.status.as_u16(),
            "body": value
        }));
    }
    let text = response.text().unwrap_or_default();
    Ok(json!({
        "ok": response.status.is_success(),
        "status": response.status.as_u16(),
        "body": truncate_utf8_bytes(&text, 8 * 1024)
    }))
}

fn is_safe_param_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn map_remote(err: RemoteError) -> ConnectorError {
    match err {
        RemoteError::Blocked(m) | RemoteError::Network(m) => ConnectorError::Validation(m),
        RemoteError::Redirect => ConnectorError::Validation(err.message()),
        RemoteError::Timeout => ConnectorError::Provider("OpenAPI request timed out".into()),
        RemoteError::TooLarge => ConnectorError::Validation("OpenAPI response is too large".into()),
        RemoteError::Http(code) => ConnectorError::Provider(format!("OpenAPI HTTP {code}")),
    }
}

pub fn attach_operation_routing(tools: &mut [InstallToolDraft], import: &OpenApiImport) {
    let _ = (tools, import);
}

/// Rebuild routing metadata into stored schemas so execute does not trust the model.
pub fn with_routing(schema: Value, method: &str, path: &str) -> Value {
    let mut obj = schema.as_object().cloned().unwrap_or_default();
    obj.insert("x-elsewhereMethod".into(), json!(method));
    obj.insert("x-elsewherePath".into(), json!(path));
    Value::Object(obj)
}

pub fn parse_openapi_for_tests(
    doc: &Value,
    document_url: &str,
) -> Result<OpenApiImport, ConnectorError> {
    // Tests that only need parsing still have to validate the server URL through the caller.
    let _ = document_url;
    let mut fake_used = std::collections::HashSet::new();
    let paths = doc
        .get("paths")
        .and_then(|v| v.as_object())
        .ok_or_else(|| ConnectorError::Validation("missing paths".into()))?;
    let mut tools = Vec::new();
    for (path, item) in paths {
        let Some(item) = item.as_object() else {
            continue;
        };
        for (method_name, spec) in item {
            let Some(method) = parse_method(method_name) else {
                continue;
            };
            let Some(op) = spec.as_object() else {
                continue;
            };
            let mut operation = operation_from_spec(path, method.clone(), op, &mut fake_used)?;
            operation.input_schema = with_routing(operation.input_schema, method.as_str(), path);
            let remote_name = operation.remote_name;
            tools.push(InstallToolDraft {
                display_name: remote_name.clone(),
                remote_name,
                description: operation.description,
                input_schema: operation.input_schema,
                read_only: operation.read_only,
            });
        }
    }
    let server = doc
        .get("servers")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.get("url"))
        .and_then(|v| v.as_str())
        .unwrap_or("https://api.example.com");
    Ok(OpenApiImport {
        base_url: server.to_string(),
        tools,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_id_when_operation_id_missing() {
        let doc = json!({
            "servers": [{ "url": "https://api.example.com" }],
            "paths": {
                "/users/{id}": {
                    "get": {
                        "parameters": [
                            { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }
                        ]
                    }
                }
            }
        });
        let imported = parse_openapi_for_tests(&doc, "https://example.com/openapi.json").unwrap();
        assert_eq!(imported.tools.len(), 1);
        assert_eq!(imported.tools[0].remote_name, "get_users_id");
        assert!(imported.tools[0].read_only);
    }

    #[tokio::test]
    async fn execute_uses_stored_route_and_rejects_header_overrides() {
        let mock = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/users/abc"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(json!({ "id": "abc" })),
            )
            .mount(&mock)
            .await;
        let client = crate::connectors::remote::RemoteHttpClient::new(
            crate::connectors::remote::RemotePolicy::for_tests(),
        );
        let schema = with_routing(
            json!({
                "type": "object",
                "properties": { "id": { "type": "string" } },
                "required": ["id"],
                "additionalProperties": false
            }),
            "GET",
            "/users/{id}",
        );
        let result = execute_openapi_operation(
            &client,
            &mock.uri(),
            "get_users_id",
            &schema,
            &json!({ "id": "abc" }),
            None,
        )
        .await
        .unwrap();
        assert_eq!(result.get("ok"), Some(&json!(true)));

        let err = execute_openapi_operation(
            &client,
            &mock.uri(),
            "get_users_id",
            &schema,
            &json!({
                "id": "abc",
                "headers": { "Authorization": "Bearer stolen" }
            }),
            None,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ConnectorError::Validation(_)));
    }

    #[test]
    fn forbids_authorization_header_override() {
        assert!(is_forbidden_header("Authorization"));
        assert!(is_forbidden_header("Cookie"));
        assert!(is_forbidden_header("X-Forwarded-For"));
        assert!(!is_forbidden_header("X-Request-Id"));
    }

    #[test]
    fn rejects_model_header_object() {
        let err = reject_model_headers(&json!({
            "headers": { "Authorization": "Bearer stolen" }
        }))
        .unwrap_err();
        assert!(matches!(err, ConnectorError::Validation(_)));
    }
}
