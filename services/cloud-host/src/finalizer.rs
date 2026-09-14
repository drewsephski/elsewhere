use agent_core::{AgentEvent, EventSink, MessageStatus, RunStore};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::events::cloud_event_sink::CloudEventSink;

pub struct HostFinalizer {
    store: Arc<dyn RunStore>,
    events: Arc<CloudEventSink>,
    request_id: String,
    assistant_message_id: String,
}

impl HostFinalizer {
    pub fn new(
        store: Arc<dyn RunStore>,
        events: Arc<CloudEventSink>,
        request_id: String,
        assistant_message_id: String,
    ) -> Self {
        Self {
            store,
            events,
            request_id,
            assistant_message_id,
        }
    }

    pub async fn finalize_host_failure(
        &self,
        error_code: &str,
        sanitized_message: &str,
        step_count: i64,
    ) -> Result<(), String> {
        let partial = self
            .store
            .get_assistant_message_body(&self.assistant_message_id)
            .await
            .map_err(|e| e.to_string())?;

        self.store
            .update_assistant_message(
                &self.assistant_message_id,
                &partial,
                MessageStatus::Error,
                Some(sanitized_message),
            )
            .await
            .map_err(|e| e.to_string())?;

        self.store
            .update_run(&self.request_id, "failed", Some(error_code), step_count)
            .await
            .map_err(|e| e.to_string())?;

        let status_payload = json!({
            "status": "failed",
            "detail": sanitized_message,
            "code": error_code,
        });
        self.persist_and_emit("status", &status_payload).await?;

        let terminal_payload = json!({
            "eventType": "error",
            "error": sanitized_message,
            "fullContent": partial,
            "code": error_code,
            "stepCount": step_count,
        });
        self.persist_and_emit("terminal", &terminal_payload).await?;

        self.events
            .emit(AgentEvent::RunFailed {
                code: error_code.to_string(),
                message: sanitized_message.to_string(),
                step_count,
            })
            .map_err(|e| e.to_string())?;
        self.events
            .emit(AgentEvent::Terminal {
                event_type: "error".into(),
                error: Some(sanitized_message.to_string()),
                full_content: Some(partial),
            })
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn finalize_host_interrupted(
        &self,
        error_code: &str,
        detail: &str,
        step_count: i64,
    ) -> Result<(), String> {
        let partial = self
            .store
            .get_assistant_message_body(&self.assistant_message_id)
            .await
            .map_err(|e| e.to_string())?;

        self.store
            .update_assistant_message(
                &self.assistant_message_id,
                &partial,
                MessageStatus::Interrupted,
                None,
            )
            .await
            .map_err(|e| e.to_string())?;

        self.store
            .update_run(
                &self.request_id,
                "interrupted",
                Some(error_code),
                step_count,
            )
            .await
            .map_err(|e| e.to_string())?;

        let status_payload = json!({
            "status": "interrupted",
            "detail": detail,
            "code": error_code,
        });
        self.persist_and_emit("status", &status_payload).await?;

        let terminal_payload = json!({
            "eventType": "interrupted",
            "error": detail,
            "fullContent": partial,
            "code": error_code,
            "stepCount": step_count,
        });
        self.persist_and_emit("terminal", &terminal_payload).await?;

        self.events
            .emit(AgentEvent::Terminal {
                event_type: "interrupted".into(),
                error: Some(detail.to_string()),
                full_content: Some(partial),
            })
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn finalize_host_cancelled(&self) -> Result<(), String> {
        let partial = self
            .store
            .get_assistant_message_body(&self.assistant_message_id)
            .await
            .map_err(|e| e.to_string())?;
        self.store
            .update_assistant_message(
                &self.assistant_message_id,
                &partial,
                MessageStatus::Cancelled,
                None,
            )
            .await
            .map_err(|e| e.to_string())?;
        self.store
            .update_run(&self.request_id, "cancelled", None, 0)
            .await
            .map_err(|e| e.to_string())?;
        self.persist_and_emit("cancelled", &json!({"status":"cancelled"}))
            .await
    }

    pub async fn finalize_run_timeout(&self, step_count: i64) -> Result<(), String> {
        self.finalize_host_interrupted("run_timeout", "run timed out", step_count)
            .await
    }

    async fn persist_and_emit(&self, event_type: &str, payload: &Value) -> Result<(), String> {
        let receipt = self
            .store
            .append_run_event(&self.request_id, event_type, payload)
            .await
            .map_err(|e| e.to_string())?;
        self.events
            .emit_durable(receipt.id, event_type, payload)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Sanitize external errors before exposing to clients or assistant messages.
pub fn sanitize_host_error(message: &str) -> String {
    crate::redact::redact_secrets(message)
}
