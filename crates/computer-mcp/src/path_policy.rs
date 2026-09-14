use crate::error::ComputerMcpError;

pub fn require_workspace_path(path: &str) -> Result<(), ComputerMcpError> {
    if path == "/workspace" || path.starts_with("/workspace/") {
        return Ok(());
    }
    Err(ComputerMcpError::SandboxRejected(format!(
        "path must be under /workspace, got {path}"
    )))
}
