use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::Notify;

use crate::connectors::github_access::ConnectorNeedResolution;

struct WaitEntry {
    notify: Notify,
    resolved: std::sync::Mutex<Option<ConnectorNeedResolution>>,
}

impl WaitEntry {
    fn resolved(&self) -> Option<ConnectorNeedResolution> {
        *self.resolved.lock().expect("connector need wait lock")
    }

    fn set_resolved(&self, resolution: ConnectorNeedResolution) {
        *self.resolved.lock().expect("connector need wait lock") = Some(resolution);
        self.notify.notify_waiters();
    }
}

pub struct ConnectorNeedWaitRegistry {
    waiters: DashMap<String, Arc<WaitEntry>>,
}

impl Default for ConnectorNeedWaitRegistry {
    fn default() -> Self {
        Self {
            waiters: DashMap::new(),
        }
    }
}

impl ConnectorNeedWaitRegistry {
    pub fn subscribe(&self, need_id: &str) -> ConnectorNeedWaitHandle {
        let entry = self
            .waiters
            .entry(need_id.to_string())
            .or_insert_with(|| {
                Arc::new(WaitEntry {
                    notify: Notify::new(),
                    resolved: std::sync::Mutex::new(None),
                })
            })
            .clone();
        ConnectorNeedWaitHandle { entry }
    }

    pub fn notify_recheck(&self, need_id: &str) -> bool {
        if let Some(entry) = self.waiters.get(need_id) {
            entry.notify.notify_waiters();
            return true;
        }
        false
    }

    pub fn resolve(&self, need_id: &str, resolution: ConnectorNeedResolution) -> bool {
        if let Some((_, entry)) = self.waiters.remove(need_id) {
            entry.set_resolved(resolution);
            return true;
        }
        false
    }

    pub fn drop_waiter(&self, need_id: &str) {
        self.waiters.remove(need_id);
    }
}

#[derive(Clone)]
pub struct ConnectorNeedWaitHandle {
    entry: Arc<WaitEntry>,
}

impl ConnectorNeedWaitHandle {
    pub fn resolved(&self) -> Option<ConnectorNeedResolution> {
        self.entry.resolved()
    }

    pub async fn notified(&self) {
        self.entry.notify.notified().await;
    }

    pub fn wake(&self) {
        self.entry.notify.notify_waiters();
    }
}
