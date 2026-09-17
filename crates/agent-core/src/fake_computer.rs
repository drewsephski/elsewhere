//! In-memory `AgentComputer` for tests and MCP proofs.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use crate::computer::{AgentComputer, ComputerError, ComputerInfo, ExecResult, WorkspaceEntry};

const MAX_CAPTURE_BYTES: usize = 256 * 1024;

#[derive(Debug, Default)]
pub struct FakeAgentComputer {
    inner: Mutex<FakeState>,
}

#[derive(Debug, Default)]
struct FakeState {
    ready: bool,
    ensure_ready_calls: usize,
    transient_failures_remaining: usize,
    ensure_ready_delay: Option<Duration>,
    files: HashMap<String, Vec<u8>>,
    listings: HashMap<String, Vec<WorkspaceEntry>>,
    exec_results: HashMap<String, ExecResult>,
    reject_non_workspace_paths: bool,
    write_file_calls: usize,
    exec_calls: usize,
}

impl FakeAgentComputer {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(FakeState {
                ready: true,
                reject_non_workspace_paths: true,
                ..Default::default()
            }),
        }
    }

    pub fn with_listing(mut self, path: &str, entries: Vec<WorkspaceEntry>) -> Self {
        self.inner
            .get_mut()
            .unwrap()
            .listings
            .insert(path.to_string(), entries);
        self
    }

    pub fn allow_any_path(mut self) -> Self {
        self.inner.get_mut().unwrap().reject_non_workspace_paths = false;
        self
    }

    pub fn ensure_ready_calls(&self) -> usize {
        self.inner.lock().unwrap().ensure_ready_calls
    }

    pub fn set_exec_result(mut self, command: &str, result: ExecResult) -> Self {
        self.inner
            .get_mut()
            .unwrap()
            .exec_results
            .insert(command.to_string(), result);
        self
    }

    pub fn with_transient_readiness_failures(mut self, count: usize) -> Self {
        self.inner.get_mut().unwrap().transient_failures_remaining = count;
        self
    }

    pub fn with_ensure_ready_delay(mut self, delay: Duration) -> Self {
        self.inner.get_mut().unwrap().ensure_ready_delay = Some(delay);
        self
    }

    pub fn write_file_calls(&self) -> usize {
        self.inner.lock().unwrap().write_file_calls
    }

    pub fn exec_calls(&self) -> usize {
        self.inner.lock().unwrap().exec_calls
    }

    pub fn file_contents(&self, path: &str) -> Option<Vec<u8>> {
        self.inner.lock().unwrap().files.get(path).cloned()
    }
}

fn ensure_workspace_path(path: &str, enforce: bool) -> Result<(), ComputerError> {
    if !enforce {
        return Ok(());
    }
    if path == "/workspace" || path.starts_with("/workspace/") {
        return Ok(());
    }
    Err(ComputerError::SandboxRejected(format!(
        "path must be under /workspace, got {path}"
    )))
}

#[async_trait]
impl AgentComputer for FakeAgentComputer {
    async fn ensure_ready(&self) -> Result<ComputerInfo, ComputerError> {
        let delay = self.inner.lock().unwrap().ensure_ready_delay;
        if let Some(delay) = delay {
            tokio::time::sleep(delay).await;
        }
        let mut state = self.inner.lock().unwrap();
        state.ensure_ready_calls += 1;
        if state.transient_failures_remaining > 0 {
            state.transient_failures_remaining -= 1;
            return Err(ComputerError::GuestUnavailable(
                "transient readiness failure".into(),
            ));
        }
        if !state.ready {
            return Err(ComputerError::NotProvisioned);
        }
        Ok(ComputerInfo {
            ready: true,
            protocol_version: 1,
            detail: Some("fake".into()),
        })
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<WorkspaceEntry>, ComputerError> {
        let state = self.inner.lock().unwrap();
        ensure_workspace_path(path, state.reject_non_workspace_paths)?;
        Ok(state.listings.get(path).cloned().unwrap_or_default())
    }

    async fn read_file(&self, path: &str) -> Result<Vec<u8>, ComputerError> {
        let state = self.inner.lock().unwrap();
        ensure_workspace_path(path, state.reject_non_workspace_paths)?;
        state
            .files
            .get(path)
            .cloned()
            .ok_or_else(|| ComputerError::ExecutionFailed(format!("file not found: {path}")))
    }

    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), ComputerError> {
        let mut state = self.inner.lock().unwrap();
        ensure_workspace_path(path, state.reject_non_workspace_paths)?;
        state.write_file_calls += 1;
        state.files.insert(path.to_string(), data.to_vec());
        Ok(())
    }

    async fn exec(&self, command: &str) -> Result<ExecResult, ComputerError> {
        let mut state = self.inner.lock().unwrap();
        state.exec_calls += 1;
        if let Some(result) = state.exec_results.get(command) {
            return Ok(truncate_exec(result.clone()));
        }
        if let Some(path) = command.strip_prefix("cat ") {
            if let Some(bytes) = state.files.get(path) {
                return Ok(truncate_exec(ExecResult {
                    ok: true,
                    stdout: String::from_utf8_lossy(bytes).into_owned(),
                    stderr: String::new(),
                    exit_code: 0,
                }));
            }
        }
        Ok(truncate_exec(ExecResult {
            ok: true,
            stdout: format!("executed: {command}"),
            stderr: String::new(),
            exit_code: 0,
        }))
    }
}

fn truncate_exec(mut result: ExecResult) -> ExecResult {
    if result.stdout.len() > MAX_CAPTURE_BYTES {
        result.stdout.truncate(MAX_CAPTURE_BYTES);
        result.stdout.push_str("\n…[truncated]");
    }
    if result.stderr.len() > MAX_CAPTURE_BYTES {
        result.stderr.truncate(MAX_CAPTURE_BYTES);
        result.stderr.push_str("\n…[truncated]");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_outside_workspace() {
        let computer = FakeAgentComputer::new();
        let err = computer.list_dir("/etc").await.unwrap_err();
        assert_eq!(
            err,
            ComputerError::SandboxRejected("path must be under /workspace, got /etc".into())
        );
    }
}
