use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::events::RuntimeError;
use crate::input::{MessageRole, MessageStatus};

#[derive(Debug, Clone)]
pub struct StructuredMessageInput {
    pub conversation_id: String,
    pub role: MessageRole,
    pub kind: String,
    pub body: String,
    pub status: MessageStatus,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedMessage {
    pub id: String,
    pub conversation_id: String,
    pub kind: String,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct CreateRunParams {
    pub conversation_id: String,
    pub request_id: String,
    pub bot_id: String,
    pub model: String,
    pub computer_id: Option<String>,
}

pub trait RunStore: Send + Sync {
    fn create_run(&self, params: CreateRunParams) -> Result<String, RuntimeError>;

    fn append_run_event(
        &self,
        request_id: &str,
        event_type: &str,
        payload: &Value,
    ) -> Result<(), RuntimeError>;

    fn persist_structured_message(
        &self,
        input: StructuredMessageInput,
    ) -> Result<PersistedMessage, RuntimeError>;

    fn update_assistant_message(
        &self,
        message_id: &str,
        body: &str,
        status: MessageStatus,
        error_message: Option<&str>,
    ) -> Result<(), RuntimeError>;

    fn update_run(
        &self,
        request_id: &str,
        status: &str,
        error_code: Option<&str>,
        step_count: i64,
    ) -> Result<(), RuntimeError>;

    fn touch_conversation_and_bot(
        &self,
        conversation_id: &str,
        bot_id: &str,
    ) -> Result<(), RuntimeError>;

    fn get_assistant_message_body(&self, message_id: &str) -> Result<String, RuntimeError>;
}
