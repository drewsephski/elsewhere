use agent_core::{AgentEvent, EventSink, RuntimeError};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::broadcast;

const EVENT_CHANNEL_CAPACITY: usize = 256;

#[derive(Debug, Clone, Serialize)]
pub struct LiveRunEvent {
    pub id: i64,
    pub event_type: String,
    pub payload: Value,
}

pub struct CloudEventSink {
    sender: broadcast::Sender<LiveRunEvent>,
}

impl CloudEventSink {
    pub fn new() -> (Self, broadcast::Receiver<LiveRunEvent>) {
        let (sender, receiver) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        (Self { sender }, receiver)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LiveRunEvent> {
        self.sender.subscribe()
    }

    pub fn publish_durable(&self, id: i64, event_type: &str, payload: Value) {
        let _ = self.sender.send(LiveRunEvent {
            id,
            event_type: event_type.to_string(),
            payload,
        });
    }
}

impl EventSink for CloudEventSink {
    fn emit(&self, event: AgentEvent) -> Result<(), RuntimeError> {
        let _ = event;
        Ok(())
    }

    fn emit_durable(
        &self,
        event_id: i64,
        event_type: &str,
        payload: &Value,
    ) -> Result<(), RuntimeError> {
        self.publish_durable(event_id, event_type, payload.clone());
        Ok(())
    }
}
