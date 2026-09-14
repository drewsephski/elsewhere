use serde::{Deserialize, Serialize};
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
            ComputerError::Cancelled => "cancelled",
        }
    }
}

/// Portable abstraction for an agent's Linux computer (local VM or future cloud sandbox).
pub trait AgentComputer: Send + Sync {
    fn ensure_ready(&self) -> Result<ComputerInfo, ComputerError>;

    fn list_dir(&self, path: &str) -> Result<Vec<WorkspaceEntry>, ComputerError>;

    fn read_file(&self, path: &str) -> Result<Vec<u8>, ComputerError>;

    fn write_file(&self, path: &str, data: &[u8]) -> Result<(), ComputerError>;

    fn exec(&self, command: &str) -> Result<ExecResult, ComputerError>;
}
