use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use tokio::sync::broadcast;

use crate::events::cloud_event_sink::{CloudEventSink, LiveRunEvent};

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

    /// Returns true when this caller should start execution for the run.
    pub fn try_begin_run(&self, run_id: &str, active: ActiveRun) -> bool {
        match self.inner.entry(run_id.to_string()) {
            Entry::Occupied(_) => false,
            Entry::Vacant(vacant) => {
                vacant.insert(active);
                true
            }
        }
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

    pub fn subscribe_live(&self, run_id: &str) -> Option<broadcast::Receiver<LiveRunEvent>> {
        self.inner
            .get(run_id)
            .map(|entry| entry.events.subscribe())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    fn dummy_active() -> ActiveRun {
        let (events, _) = CloudEventSink::new();
        ActiveRun {
            request_id: "req".into(),
            cancel: Arc::new(AtomicBool::new(false)),
            events: Arc::new(events),
            started_at: Instant::now(),
        }
    }

    #[test]
    fn try_begin_run_allows_only_one_winner() {
        let registry = RunRegistry::default();
        let run_id = "run-1";
        let winners = std::sync::Mutex::new(0usize);

        std::thread::scope(|scope| {
            for _ in 0..32 {
                scope.spawn(|| {
                    if registry.try_begin_run(run_id, dummy_active()) {
                        *winners.lock().unwrap() += 1;
                    }
                });
            }
        });

        assert_eq!(*winners.lock().unwrap(), 1);
        assert!(registry.get(run_id).is_some());
    }
}
