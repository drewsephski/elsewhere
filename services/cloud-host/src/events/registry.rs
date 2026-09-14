use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

use dashmap::DashMap;
use tokio::sync::broadcast;

use agent_core::AgentEvent;
use crate::events::cloud_event_sink::CloudEventSink;

pub struct ActiveRun {
    pub request_id: String,
    pub cancel: Arc<AtomicBool>,
    pub events: Arc<CloudEventSink>,
    pub started_at: Instant,
}

#[derive(Default)]
pub struct RunRegistry {
    inner: DashMap<String, ActiveRun>,
}

impl RunRegistry {
    pub fn insert(&self, run_id: String, active: ActiveRun) {
        self.inner.insert(run_id, active);
    }

    pub fn get(&self, run_id: &str) -> Option<dashmap::mapref::one::Ref<'_, String, ActiveRun>> {
        self.inner.get(run_id)
    }

    pub fn remove(&self, run_id: &str) {
        self.inner.remove(run_id);
    }

    pub fn cancel(&self, run_id: &str) -> bool {
        if let Some(entry) = self.inner.get(run_id) {
            entry.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            return true;
        }
        false
    }

    pub fn subscribe_live(&self, run_id: &str) -> Option<broadcast::Receiver<AgentEvent>> {
        self.inner
            .get(run_id)
            .map(|entry| entry.events.subscribe())
    }
}
