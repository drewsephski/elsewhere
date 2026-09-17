use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerInfo {
    pub ready: bool,
    pub protocol_version: u32,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecResult {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ComputerError {
    #[error("computer not provisioned")]
    NotProvisioned,
    #[error("computer boot failed: {0}")]
    BootFailed(String),
    #[error("guest unavailable: {0}")]
    GuestUnavailable(String),
    #[error("malformed arguments: {0}")]
    MalformedArguments(String),
    #[error("sandbox rejected: {0}")]
    SandboxRejected(String),
    #[error("execution failed: {0}")]
    ExecutionFailed(String),
    #[error("computer operation outcome is ambiguous: {0}")]
    AmbiguousOutcome(String),
    #[error("cancelled")]
    Cancelled,
}

impl ComputerError {
    pub fn code(&self) -> &'static str {
        match self {
            ComputerError::NotProvisioned => "vm_not_provisioned",
            ComputerError::BootFailed(_) => "vm_boot_failed",
            ComputerError::GuestUnavailable(_) => "guest_unavailable",
            ComputerError::MalformedArguments(_) => "malformed_tool_arguments",
            ComputerError::SandboxRejected(_) => "tool_rejected_by_sandbox",
            ComputerError::ExecutionFailed(_) => "tool_execution_failed",
            ComputerError::AmbiguousOutcome(_) => "computer_operation_ambiguous",
            ComputerError::Cancelled => "cancelled",
        }
    }

    pub fn is_ambiguous(&self) -> bool {
        matches!(self, ComputerError::AmbiguousOutcome(_))
    }
}

/// Portable abstraction for an agent's Linux computer (local VM or future cloud sandbox).
#[async_trait]
pub trait AgentComputer: Send + Sync {
    async fn ensure_ready(&self) -> Result<ComputerInfo, ComputerError>;

    /// Clear any host-side readiness cache after guest/provider errors.
    fn invalidate_cached_readiness(&self) {}

    async fn list_dir(&self, path: &str) -> Result<Vec<WorkspaceEntry>, ComputerError>;

    async fn read_file(&self, path: &str) -> Result<Vec<u8>, ComputerError>;

    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), ComputerError>;

    async fn exec(&self, command: &str) -> Result<ExecResult, ComputerError>;

    /// Headless browser automation inside the agent computer (Sprite guest). Default: unavailable.
    async fn browser_invoke(&self, _action: &str, _args: &Value) -> Result<Value, ComputerError> {
        Err(ComputerError::SandboxRejected(
            "browser automation is not available on this computer".into(),
        ))
    }

    /// Monotonic revision for workspace tree invalidation (0 when unsupported).
    fn workspace_revision(&self) -> u64 {
        0
    }

    /// Called after a successful tool dispatch that may have changed workspace listings.
    fn record_workspace_mutation(&self, _tool_name: &str, _result: &Value) {}
}

/// Shared revision counter for computers that expose workspace invalidation.
#[derive(Debug, Default)]
pub struct WorkspaceRevisionCounter(AtomicU64);

impl WorkspaceRevisionCounter {
    pub fn get(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }

    pub fn bump(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst) + 1
    }
}

pub fn workspace_tool_mutation(tool_name: &str, result: &Value) -> bool {
    if !matches!(
        tool_name,
        "workspace_write" | "workspace_exec" | "browser_screenshot" | "browser_download"
    ) {
        return false;
    }
    result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false)
}
