use dashmap::DashMap;
use tokio::sync::oneshot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalResolution {
    Approved,
    Denied { reason: String },
    Cancelled { reason: String },
    Expired,
}

pub struct ApprovalWaitRegistry {
    waiters: DashMap<String, oneshot::Sender<ApprovalResolution>>,
}

impl Default for ApprovalWaitRegistry {
    fn default() -> Self {
        Self {
            waiters: DashMap::new(),
        }
    }
}

impl ApprovalWaitRegistry {
    pub fn register(&self, approval_id: &str) -> oneshot::Receiver<ApprovalResolution> {
        let (tx, rx) = oneshot::channel();
        self.waiters.insert(approval_id.to_string(), tx);
        rx
    }

    pub fn notify(&self, approval_id: &str, resolution: ApprovalResolution) -> bool {
        if let Some((_, tx)) = self.waiters.remove(approval_id) {
            let _ = tx.send(resolution);
            return true;
        }
        false
    }

    pub fn drop_waiter(&self, approval_id: &str) {
        self.waiters.remove(approval_id);
    }
}
