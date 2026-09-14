//! Per-run cache for `ensure_ready` so multiple tools do not repeat Sprite probes.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::{Arc, Mutex};

use crate::computer::{
    AgentComputer, ComputerError, ComputerInfo, ExecResult, WorkspaceEntry,
};

pub struct ReadinessCachedComputer {
    inner: Arc<dyn AgentComputer>,
    cache: Mutex<Option<Result<ComputerInfo, ComputerError>>>,
}

impl ReadinessCachedComputer {
    pub fn new(inner: Arc<dyn AgentComputer>) -> Self {
        Self {
            inner,
            cache: Mutex::new(None),
        }
    }

    pub fn invalidate_readiness(&self) {
        *self.cache.lock().expect("readiness cache poisoned") = None;
    }

}

#[async_trait]
impl AgentComputer for ReadinessCachedComputer {
    fn invalidate_cached_readiness(&self) {
        self.invalidate_readiness();
    }

    async fn ensure_ready(&self) -> Result<ComputerInfo, ComputerError> {
        if let Some(cached) = self.cache.lock().expect("readiness cache poisoned").clone() {
            return cached;
        }
        let result = self.inner.ensure_ready().await;
        *self.cache.lock().expect("readiness cache poisoned") = Some(result.clone());
        result
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<WorkspaceEntry>, ComputerError> {
        self.inner.list_dir(path).await
    }

    async fn read_file(&self, path: &str) -> Result<Vec<u8>, ComputerError> {
        self.inner.read_file(path).await
    }

    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), ComputerError> {
        self.inner.write_file(path, data).await
    }

    async fn exec(&self, command: &str) -> Result<ExecResult, ComputerError> {
        self.inner.exec(command).await
    }

    async fn browser_invoke(&self, action: &str, args: &Value) -> Result<Value, ComputerError> {
        self.inner.browser_invoke(action, args).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FakeAgentComputer;

    #[tokio::test]
    async fn caches_successful_readiness_within_a_run() {
        let inner = Arc::new(FakeAgentComputer::new());
        let wrapped = ReadinessCachedComputer::new(inner.clone());
        let _ = wrapped.ensure_ready().await.expect("ready");
        let _ = wrapped.ensure_ready().await.expect("ready");
        assert_eq!(inner.ensure_ready_calls(), 1);
    }
}
