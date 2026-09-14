use agent_core::{AgentEvent, EventSink, RuntimeError};
use tauri::{AppHandle, Emitter};

use crate::commands::STREAM_EVENT;
use crate::models::{Message, MessageRole, MessageStatus, StreamEventPayload};

pub struct TauriEventSink {
    app: AppHandle,
    request_id: String,
    conversation_id: String,
    assistant_message_id: String,
}

impl TauriEventSink {
    pub fn new(
        app: AppHandle,
        request_id: String,
        conversation_id: String,
        assistant_message_id: String,
    ) -> Self {
        Self {
            app,
            request_id,
            conversation_id,
            assistant_message_id,
        }
    }

    fn emit_stream(&self, payload: StreamEventPayload) -> Result<(), RuntimeError> {
        self.app
            .emit(STREAM_EVENT, payload)
            .map_err(|e| RuntimeError::EventSink(e.to_string()))
    }
}

impl EventSink for TauriEventSink {
    fn emit(&self, event: AgentEvent) -> Result<(), RuntimeError> {
        match event {
            AgentEvent::ToolCall { message, .. }
            | AgentEvent::ToolResult { message, .. }
            | AgentEvent::StatusChanged { message, .. } => {
                if let Some(persisted) = message {
                    let message = Message {
                        id: persisted.id,
                        conversation_id: persisted.conversation_id,
                        role: MessageRole::Assistant,
                        kind: persisted.kind,
                        body: persisted.body,
                        status: MessageStatus::Complete,
                        model: None,
                        error_message: None,
                        created_at: 0,
                        updated_at: 0,
                    };
                    self.emit_stream(StreamEventPayload {
                        request_id: self.request_id.clone(),
                        event_type: "message".into(),
                        conversation_id: self.conversation_id.clone(),
                        assistant_message_id: self.assistant_message_id.clone(),
                        delta: None,
                        error: None,
                        full_content: None,
                        message: Some(message),
                    })?;
                }
            }
            AgentEvent::Terminal {
                event_type,
                error,
                full_content,
            } => {
                self.emit_stream(StreamEventPayload {
                    request_id: self.request_id.clone(),
                    event_type,
                    conversation_id: self.conversation_id.clone(),
                    assistant_message_id: self.assistant_message_id.clone(),
                    delta: None,
                    error,
                    full_content,
                    message: None,
                })?;
            }
            AgentEvent::RunStarted { .. }
            | AgentEvent::AssistantMessage { .. }
            | AgentEvent::RunCompleted { .. }
            | AgentEvent::RunFailed { .. }
            | AgentEvent::RunCancelled => {}
        }
        Ok(())
    }
}
