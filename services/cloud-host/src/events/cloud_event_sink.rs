use agent_core::{AgentEvent, EventSink, RuntimeError};
use tokio::sync::broadcast;

const EVENT_CHANNEL_CAPACITY: usize = 256;

pub struct CloudEventSink {
    sender: broadcast::Sender<AgentEvent>,
}

impl CloudEventSink {
    pub fn new() -> (Self, broadcast::Receiver<AgentEvent>) {
        let (sender, receiver) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        (Self { sender }, receiver)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.sender.subscribe()
    }
}

impl EventSink for CloudEventSink {
    fn emit(&self, event: AgentEvent) -> Result<(), RuntimeError> {
        let _ = self.sender.send(event);
        Ok(())
    }
}
