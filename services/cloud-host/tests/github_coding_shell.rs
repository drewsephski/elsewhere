//! Real shell + git under a temp `/workspace` root for GitHub coding integration tests.

use agent_core::{AgentComputer, ComputerError, ComputerInfo, ExecResult, WorkspaceEntry};
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Mutex;
use tempfile::TempDir;
use tokio::process::Command;

pub struct ShellWorkspaceComputer {
    workspace_root: TempDir,
    files: Mutex<std::collections::HashMap<String, Vec<u8>>>,
}

impl ShellWorkspaceComputer {
    pub fn new() -> Self {
        let root = tempfile::tempdir().expect("tempdir");
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace dir");
        Self {
            workspace_root: root,
            files: Mutex::new(std::collections::HashMap::new()),
        }
    }

    fn map_path(&self, path: &str) -> PathBuf {
        let suffix = path
            .strip_prefix("/workspace")
            .unwrap_or(path)
            .trim_start_matches('/');
        self.workspace_root.path().join("workspace").join(suffix)
    }
}

#[async_trait]
impl AgentComputer for ShellWorkspaceComputer {
    async fn ensure_ready(&self) -> Result<ComputerInfo, ComputerError> {
        Ok(ComputerInfo {
            ready: true,
            protocol_version: 1,
            detail: Some("shell-workspace".into()),
        })
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<WorkspaceEntry>, ComputerError> {
        let mapped = self.map_path(path);
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&mapped).map_err(|e| ComputerError::ExecutionFailed(e.to_string()))? {
            let entry = entry.map_err(|e| ComputerError::ExecutionFailed(e.to_string()))?;
            let name = entry.file_name().to_string_lossy().to_string();
            let full = entry.path().to_string_lossy().to_string();
            entries.push(WorkspaceEntry {
                name,
                path: full,
                is_dir: entry.file_type().map(|t| t.is_dir()).unwrap_or(false),
            });
        }
        Ok(entries)
    }

    async fn read_file(&self, path: &str) -> Result<Vec<u8>, ComputerError> {
        let mapped = self.map_path(path);
        std::fs::read(&mapped).map_err(|e| ComputerError::ExecutionFailed(e.to_string()))
    }

    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), ComputerError> {
        let mapped = self.map_path(path);
        if let Some(parent) = mapped.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ComputerError::ExecutionFailed(e.to_string()))?;
        }
        std::fs::write(&mapped, data).map_err(|e| ComputerError::ExecutionFailed(e.to_string()))?;
        self.files
            .lock()
            .unwrap()
            .insert(path.to_string(), data.to_vec());
        Ok(())
    }

    async fn exec(&self, command: &str) -> Result<ExecResult, ComputerError> {
        let workspace = self.workspace_root.path().join("workspace");
        let output = Command::new("bash")
            .arg("-lc")
            .arg(command)
            .current_dir(&workspace)
            .output()
            .await
            .map_err(|e| ComputerError::ExecutionFailed(e.to_string()))?;
        Ok(ExecResult {
            ok: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            exit_code: output.status.code().unwrap_or(-1),
        })
    }
}

pub fn workspace_path(computer: &ShellWorkspaceComputer, suffix: &str) -> String {
    format!("/workspace/{}", suffix.trim_start_matches('/'))
}
