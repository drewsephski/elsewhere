use crate::db::Database;
use crate::models::{MessageRole, MessageStatus};
use agent_core::{
    CreateRunParams, MessageRole as CoreRole, MessageStatus as CoreStatus, PersistedMessage,
    RunEventReceipt, RunStore, RuntimeError, StructuredMessageInput,
};
use async_trait::async_trait;
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

#[async_trait]
impl RunStore for SqliteRunStore {
    async fn create_run(&self, params: CreateRunParams) -> Result<String, RuntimeError> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let db = db.lock();
            db.create_agent_run(
                &params.conversation_id,
                &params.request_id,
                &params.bot_id,
                &params.model,
                params.computer_id.as_deref(),
            )
            .map_err(|e| RuntimeError::Store(e.to_string()))
        })
        .await
        .map_err(|e| RuntimeError::Store(e.to_string()))?
    }

    async fn append_run_event(
        &self,
        request_id: &str,
        event_type: &str,
        payload: &Value,
    ) -> Result<RunEventReceipt, RuntimeError> {
        let db = self.db.clone();
        let request_id = request_id.to_string();
        let event_type = event_type.to_string();
        let payload = payload.clone();
        tokio::task::spawn_blocking(move || {
            let db = db.lock();
            let id = db
                .append_run_event(&request_id, &event_type, &payload)
                .map_err(|e| RuntimeError::Store(e.to_string()))?;
            Ok(RunEventReceipt { id })
        })
        .await
        .map_err(|e| RuntimeError::Store(e.to_string()))?
    }

    async fn persist_structured_message(
        &self,
        input: StructuredMessageInput,
    ) -> Result<PersistedMessage, RuntimeError> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
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
            let db = db.lock();
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
        })
        .await
        .map_err(|e| RuntimeError::Store(e.to_string()))?
    }

    async fn update_assistant_message(
        &self,
        message_id: &str,
        body: &str,
        status: CoreStatus,
        error_message: Option<&str>,
    ) -> Result<(), RuntimeError> {
        let db = self.db.clone();
        let message_id = message_id.to_string();
        let body = body.to_string();
        let error_message = error_message.map(str::to_string);
        tokio::task::spawn_blocking(move || {
            let status = match status {
                CoreStatus::Pending => MessageStatus::Pending,
                CoreStatus::Streaming => MessageStatus::Streaming,
                CoreStatus::Complete => MessageStatus::Complete,
                CoreStatus::Error => MessageStatus::Error,
                CoreStatus::Cancelled => MessageStatus::Cancelled,
                CoreStatus::Interrupted => MessageStatus::Interrupted,
            };
            let db = db.lock();
            db.update_message_body_and_status(&message_id, &body, status, error_message.as_deref())
                .map_err(|e| RuntimeError::Store(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(|e| RuntimeError::Store(e.to_string()))?
    }

    async fn update_run(
        &self,
        request_id: &str,
        status: &str,
        error_code: Option<&str>,
        step_count: i64,
    ) -> Result<(), RuntimeError> {
        let db = self.db.clone();
        let request_id = request_id.to_string();
        let status = status.to_string();
        let error_code = error_code.map(str::to_string);
        tokio::task::spawn_blocking(move || {
            let db = db.lock();
            db.update_agent_run(&request_id, &status, error_code.as_deref(), step_count)
                .map_err(|e| RuntimeError::Store(e.to_string()))
        })
        .await
        .map_err(|e| RuntimeError::Store(e.to_string()))?
    }

    async fn touch_conversation_and_bot(
        &self,
        conversation_id: &str,
        bot_id: &str,
    ) -> Result<(), RuntimeError> {
        let db = self.db.clone();
        let conversation_id = conversation_id.to_string();
        let bot_id = bot_id.to_string();
        tokio::task::spawn_blocking(move || {
            let db = db.lock();
            db.touch_conversation(&conversation_id)
                .map_err(|e| RuntimeError::Store(e.to_string()))?;
            db.touch_bot(&bot_id)
                .map_err(|e| RuntimeError::Store(e.to_string()))
        })
        .await
        .map_err(|e| RuntimeError::Store(e.to_string()))?
    }

    async fn get_assistant_message_body(&self, message_id: &str) -> Result<String, RuntimeError> {
        let db = self.db.clone();
        let message_id = message_id.to_string();
        tokio::task::spawn_blocking(move || {
            let db = db.lock();
            let message = db
                .get_message(&message_id)
                .map_err(|e| RuntimeError::Store(e.to_string()))?;
            Ok(message.body)
        })
        .await
        .map_err(|e| RuntimeError::Store(e.to_string()))?
    }
}
