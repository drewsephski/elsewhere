//! Per-run cache for `ensure_ready` so multiple tools do not repeat Sprite probes.

use async_trait::async_trait;
use futures_util::FutureExt;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use crate::computer::{
    AgentComputer, ComputerError, ComputerInfo, ExecResult, WorkspaceEntry,
};

type ReadyFuture = futures_util::future::Shared<
    Pin<Box<dyn Future<Output = Result<ComputerInfo, ComputerError>> + Send>>,
>;

pub struct ReadinessCachedComputer {
    inner: Arc<dyn AgentComputer>,
    cached_success: Mutex<Option<ComputerInfo>>,
    inflight: tokio::sync::Mutex<Option<ReadyFuture>>,
}

impl ReadinessCachedComputer {
    pub fn new(inner: Arc<dyn AgentComputer>) -> Self {
        Self {
            inner,
            cached_success: Mutex::new(None),
            inflight: tokio::sync::Mutex::new(None),
        }
    }

    pub fn invalidate_readiness(&self) {
        *self.cached_success.lock().expect("readiness cache poisoned") = None;
        // Drop any in-flight probe so the next call retries.
        if let Ok(mut slot) = self.inflight.try_lock() {
            *slot = None;
        }
    }
}

#[async_trait]
impl AgentComputer for ReadinessCachedComputer {
    fn invalidate_cached_readiness(&self) {
        self.invalidate_readiness();
    }

    async fn ensure_ready(&self) -> Result<ComputerInfo, ComputerError> {
        if let Some(cached) = self.cached_success.lock().expect("readiness cache poisoned").clone() {
            return Ok(cached);
        }

        let mut slot = self.inflight.lock().await;
        if slot.is_none() {
            let inner = self.inner.clone();
            let probe = async move { inner.ensure_ready().await }.boxed().shared();
            *slot = Some(probe);
        }
        let shared = slot.as_ref().expect("readiness probe slot").clone();
        drop(slot);

        let result = shared.await;
        if result.is_ok() {
            *self.cached_success.lock().expect("readiness cache poisoned") =
                result.clone().ok();
        } else {
            *self.inflight.lock().await = None;
        }
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

    fn workspace_revision(&self) -> u64 {
        self.inner.workspace_revision()
    }

    fn record_workspace_mutation(&self, tool_name: &str, result: &Value) {
        self.inner.record_workspace_mutation(tool_name, result);
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

    #[tokio::test]
    async fn transient_readiness_failure_can_succeed_on_retry() {
        let inner = Arc::new(FakeAgentComputer::new().with_transient_readiness_failures(1));
        let wrapped = ReadinessCachedComputer::new(inner.clone());
        let first = wrapped.ensure_ready().await;
        assert!(first.is_err());
        let second = wrapped.ensure_ready().await.expect("ready on retry");
        assert!(second.ready);
        assert_eq!(inner.ensure_ready_calls(), 2);
        let _ = wrapped.ensure_ready().await.expect("cached");
        assert_eq!(inner.ensure_ready_calls(), 2);
    }

    #[tokio::test]
    async fn concurrent_first_readiness_performs_one_probe() {
        let inner = Arc::new(
            FakeAgentComputer::new().with_ensure_ready_delay(std::time::Duration::from_millis(80)),
        );
        let wrapped = ReadinessCachedComputer::new(inner.clone());
        let (a, b, c) = tokio::join!(
            wrapped.ensure_ready(),
            wrapped.ensure_ready(),
            wrapped.ensure_ready(),
        );
        assert!(a.is_ok());
        assert!(b.is_ok());
        assert!(c.is_ok());
        assert_eq!(inner.ensure_ready_calls(), 1);
    }
}
