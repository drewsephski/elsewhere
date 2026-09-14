use agent_core::{
    CreateRunParams, MessageRole as CoreRole, MessageStatus as CoreStatus, PersistedMessage,
    RunStore, RuntimeError, StructuredMessageInput,
};
use crate::db::Database;
use crate::models::{MessageRole, MessageStatus};
use parking_lot::Mutex;
use serde_json::Value;
use std::sync::Arc;

pub struct SqliteRunStore {
    db: Arc<Mutex<Database>>,
}

impl SqliteRunStore {
    pub fn new(db: Arc<Mutex<Database>>) -> Self {
        Self { db }
    }
}

impl RunStore for SqliteRunStore {
    fn create_run(&self, params: CreateRunParams) -> Result<String, RuntimeError> {
        let db = self.db.lock();
        db.create_agent_run(
            &params.conversation_id,
            &params.request_id,
            &params.bot_id,
            &params.model,
            params.computer_id.as_deref(),
        )
        .map_err(|e| RuntimeError::Store(e.to_string()))
    }

    fn append_run_event(
        &self,
        request_id: &str,
        event_type: &str,
        payload: &Value,
    ) -> Result<(), RuntimeError> {
        let db = self.db.lock();
        db.append_run_event(request_id, event_type, payload)
            .map_err(|e| RuntimeError::Store(e.to_string()))
    }

    fn persist_structured_message(
        &self,
        input: StructuredMessageInput,
    ) -> Result<PersistedMessage, RuntimeError> {
        let role = match input.role {
            CoreRole::User => MessageRole::User,
            CoreRole::Assistant => MessageRole::Assistant,
            CoreRole::System => MessageRole::System,
        };
        let status = match input.status {
            CoreStatus::Pending => MessageStatus::Pending,
            CoreStatus::Streaming => MessageStatus::Streaming,
            CoreStatus::Complete => MessageStatus::Complete,
            CoreStatus::Error => MessageStatus::Error,
            CoreStatus::Cancelled => MessageStatus::Cancelled,
            CoreStatus::Interrupted => MessageStatus::Interrupted,
        };
        let db = self.db.lock();
        let message = db
            .insert_structured_message(
                &input.conversation_id,
                role,
                &input.kind,
                &input.body,
                status,
                input.model.as_deref(),
            )
            .map_err(|e| RuntimeError::Store(e.to_string()))?;
        Ok(PersistedMessage {
            id: message.id,
            conversation_id: message.conversation_id,
            kind: message.kind,
            body: message.body,
        })
    }

    fn update_assistant_message(
        &self,
        message_id: &str,
        body: &str,
        status: CoreStatus,
        error_message: Option<&str>,
    ) -> Result<(), RuntimeError> {
        let status = match status {
            CoreStatus::Pending => MessageStatus::Pending,
            CoreStatus::Streaming => MessageStatus::Streaming,
            CoreStatus::Complete => MessageStatus::Complete,
            CoreStatus::Error => MessageStatus::Error,
            CoreStatus::Cancelled => MessageStatus::Cancelled,
            CoreStatus::Interrupted => MessageStatus::Interrupted,
        };
        let db = self.db.lock();
        db.update_message_body_and_status(message_id, body, status, error_message)
            .map_err(|e| RuntimeError::Store(e.to_string()))?;
        Ok(())
    }

    fn update_run(
        &self,
        request_id: &str,
        status: &str,
        error_code: Option<&str>,
        step_count: i64,
    ) -> Result<(), RuntimeError> {
        let db = self.db.lock();
        db.update_agent_run(request_id, status, error_code, step_count)
            .map_err(|e| RuntimeError::Store(e.to_string()))
    }

    fn touch_conversation_and_bot(
        &self,
        conversation_id: &str,
        bot_id: &str,
    ) -> Result<(), RuntimeError> {
        let db = self.db.lock();
        db.touch_conversation(conversation_id)
            .map_err(|e| RuntimeError::Store(e.to_string()))?;
        db.touch_bot(bot_id)
            .map_err(|e| RuntimeError::Store(e.to_string()))?;
        Ok(())
    }

    fn get_assistant_message_body(&self, message_id: &str) -> Result<String, RuntimeError> {
        let db = self.db.lock();
        let message = db
            .get_message(message_id)
            .map_err(|e| RuntimeError::Store(e.to_string()))?;
        Ok(message.body)
    }
}
