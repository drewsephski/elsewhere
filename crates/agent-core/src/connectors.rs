//! Provider-neutral connector tools (GitHub first; not part of AgentComputer).

use async_trait::async_trait;
use serde_json::Value;

/// Maximum serialized JSON size returned from a connector tool to the agent runtime.
pub const MAX_CONNECTOR_TOOL_RESULT_BYTES: usize = 64 * 1024;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectorError {
    NotConnected,
    Validation(String),
    Provider(String),
    Internal(String),
}

impl ConnectorError {
    pub fn code(&self) -> &'static str {
        match self {
            ConnectorError::NotConnected => "not_connected",
            ConnectorError::Validation(_) => "validation_error",
            ConnectorError::Provider(_) => "provider_error",
            ConnectorError::Internal(_) => "internal_error",
        }
    }

    pub fn message(&self) -> String {
        match self {
            ConnectorError::NotConnected => {
                "Connector is not connected for this account".into()
            }
            ConnectorError::Validation(m) => m.clone(),
            ConnectorError::Provider(m) => m.clone(),
            ConnectorError::Internal(m) => m.clone(),
        }
    }
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
        let err = bound_connector_tool_result(json!({ "content": huge }))
            .expect_err("oversized");
        assert!(matches!(err, ConnectorError::Validation(_)));
    }
}
