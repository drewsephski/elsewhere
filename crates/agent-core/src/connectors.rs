//! Provider-neutral connector tools (GitHub first; not part of AgentComputer).

use async_trait::async_trait;
use serde_json::Value;

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
