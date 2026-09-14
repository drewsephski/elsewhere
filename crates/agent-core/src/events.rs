use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::run_store::PersistedMessage;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("validation: {0}")]
    Validation(String),
    #[error("model: {0}")]
    Model(String),
    #[error("store: {0}")]
    Store(String),
    #[error("event sink: {0}")]
    EventSink(String),
    #[error("cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    RunStarted {
        request_id: String,
    },
    StatusChanged {
        status: String,
        detail: Option<String>,
        message: Option<PersistedMessage>,
    },
    ToolCall {
        tool: String,
        call_id: String,
        arguments: Value,
        message: Option<PersistedMessage>,
    },
    ToolResult {
        tool: String,
        call_id: String,
        ok: bool,
        output: String,
        message: Option<PersistedMessage>,
    },
    AssistantMessage {
        content: String,
    },
    RunCompleted {
        step_count: i64,
    },
    RunFailed {
        code: String,
        message: String,
        step_count: i64,
    },
    RunCancelled,
    Terminal {
        event_type: String,
        error: Option<String>,
        full_content: Option<String>,
    },
}

pub trait EventSink: Send + Sync {
    fn emit(&self, event: AgentEvent) -> Result<(), RuntimeError>;

    /// Invoked after a row is persisted in `run_events` (cloud SSE cursor).
    fn emit_durable(
        &self,
        event_id: i64,
        event_type: &str,
        payload: &Value,
    ) -> Result<(), RuntimeError> {
        let _ = (event_id, event_type, payload);
        Ok(())
    }
}
