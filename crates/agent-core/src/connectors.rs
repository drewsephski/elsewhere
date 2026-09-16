//! Provider-neutral connector tools (GitHub plus installed MCP/OpenAPI apps).
//! Execution stays outside `AgentComputer`.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::subagent::truncate_utf8_bytes;

/// Maximum serialized JSON size returned from a connector tool to the agent runtime.
pub const MAX_CONNECTOR_TOOL_RESULT_BYTES: usize = 64 * 1024;

pub const CONNECTED_APPS_SEARCH_TOOL: &str = "connected_apps_search_tools";
pub const CONNECTED_APPS_LOAD_TOOL: &str = "connected_apps_load_tool";
pub const CONNECTED_APPS_EXECUTE_TOOL: &str = "connected_apps_execute_tool";

pub const MAX_CONNECTED_APP_SEARCH_LIMIT: u32 = 20;
pub const MAX_CONNECTED_APP_DESCRIPTION_CHARS: usize = 280;

/// Reject connector tool payloads that would bloat an agent turn.
pub fn bound_connector_tool_result(value: Value) -> Result<Value, ConnectorError> {
    let serialized = serde_json::to_string(&value).map_err(|e| {
        ConnectorError::Internal(format!("connector result is not JSON-serializable: {e}"))
    })?;
    if serialized.len() <= MAX_CONNECTOR_TOOL_RESULT_BYTES {
        return Ok(value);
    }
    Err(ConnectorError::Validation(format!(
        "connector tool result exceeds {} bytes (got {})",
        MAX_CONNECTOR_TOOL_RESULT_BYTES,
        serialized.len()
    )))
}

/// Bound untrusted remote output, truncating on UTF-8 boundaries instead of failing.
pub fn truncate_connector_tool_result(value: Value) -> Result<Value, ConnectorError> {
    let serialized = serde_json::to_string(&value).map_err(|e| {
        ConnectorError::Internal(format!("connector result is not JSON-serializable: {e}"))
    })?;
    if serialized.len() <= MAX_CONNECTOR_TOOL_RESULT_BYTES {
        return Ok(value);
    }
    let truncated = truncate_utf8_bytes(
        &serialized,
        MAX_CONNECTOR_TOOL_RESULT_BYTES.saturating_sub(64),
    );
    Ok(json!({
        "ok": true,
        "truncated": true,
        "content": truncated,
    }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectorError {
    NotConnected,
    ReconnectRequired,
    NotFound,
    Validation(String),
    Provider(String),
    Internal(String),
}

impl ConnectorError {
    pub fn code(&self) -> &'static str {
        match self {
            ConnectorError::NotConnected => "not_connected",
            ConnectorError::ReconnectRequired => "reconnect_required",
            ConnectorError::NotFound => "not_found",
            ConnectorError::Validation(_) => "validation_error",
            ConnectorError::Provider(_) => "provider_error",
            ConnectorError::Internal(_) => "internal_error",
        }
    }

    pub fn message(&self) -> String {
        match self {
            ConnectorError::NotConnected => "Connector is not connected for this account".into(),
            ConnectorError::ReconnectRequired => {
                "This connected app needs to be reconnected by the owner".into()
            }
            ConnectorError::NotFound => "Connected app tool was not found".into(),
            ConnectorError::Validation(m) => m.clone(),
            ConnectorError::Provider(m) => m.clone(),
            ConnectorError::Internal(m) => m.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorToolRoute {
    pub install_id: String,
    pub remote_tool_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorToolDefinition {
    pub id: String,
    pub install_id: String,
    pub name: String,
    pub source: String,
    pub description: String,
    pub input_schema: Value,
    pub read_only: bool,
    pub kind: String,
}

/// Host implements this for owner-scoped connector API access during agent runs.
#[async_trait]
pub trait AgentConnectors: Send + Sync {
    async fn dispatch_connector_tool(
        &self,
        owner_id: &str,
        tool_name: &str,
        arguments: &Value,
    ) -> Result<Value, ConnectorError>;

    async fn search_connected_app_tools(
        &self,
        owner_id: &str,
        query: Option<&str>,
        source: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Value, ConnectorError>;

    async fn load_connected_app_tool(
        &self,
        owner_id: &str,
        tool_id: &str,
    ) -> Result<ConnectorToolDefinition, ConnectorError>;

    async fn execute_connected_app_tool(
        &self,
        owner_id: &str,
        tool_id: &str,
        arguments: &Value,
    ) -> Result<Value, ConnectorError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bound_connector_tool_result_accepts_small_payload() {
        let value = json!({ "ok": true, "items": [1, 2, 3] });
        let bounded = bound_connector_tool_result(value.clone()).expect("bounded");
        assert_eq!(bounded, value);
    }

    #[test]
    fn bound_connector_tool_result_rejects_oversized_payload() {
        let huge = "x".repeat(MAX_CONNECTOR_TOOL_RESULT_BYTES + 1);
        let err = bound_connector_tool_result(json!({ "content": huge })).expect_err("oversized");
        assert!(matches!(err, ConnectorError::Validation(_)));
    }

    #[test]
    fn truncate_connector_tool_result_preserves_utf8() {
        let huge = format!("{}🎉", "x".repeat(MAX_CONNECTOR_TOOL_RESULT_BYTES));
        let truncated =
            truncate_connector_tool_result(json!({ "content": huge })).expect("truncated");
        assert_eq!(truncated.get("truncated"), Some(&json!(true)));
        let content = truncated
            .get("content")
            .and_then(|v| v.as_str())
            .expect("content");
        assert!(std::str::from_utf8(content.as_bytes()).is_ok());
        assert!(content.len() <= MAX_CONNECTOR_TOOL_RESULT_BYTES);
    }
}
