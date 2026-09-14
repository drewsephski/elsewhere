use agent_core::{
    AgentComputer, ComputerError, ComputerInfo, ExecResult, WorkspaceEntry,
};
use crate::vm::{GuestRequest, VirtualMachineManager};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;

const GUEST_READY_TIMEOUT: Duration = Duration::from_secs(120);

pub struct LocalMacComputer {
    vm: Arc<VirtualMachineManager>,
}

impl LocalMacComputer {
    pub fn new(vm: Arc<VirtualMachineManager>) -> Self {
        Self { vm }
    }
}

impl AgentComputer for LocalMacComputer {
    fn ensure_ready(&self) -> Result<ComputerInfo, ComputerError> {
        if !self.vm.layout().vm_config_path().exists() {
            return Err(ComputerError::NotProvisioned);
        }
        self.vm.ensure_guest_ready(GUEST_READY_TIMEOUT).map_err(|err| {
            if err.contains("not provisioned") {
                ComputerError::NotProvisioned
            } else if err.contains("boot") || err.contains("start") {
                ComputerError::BootFailed(err)
            } else {
                ComputerError::GuestUnavailable(err)
            }
        })?;
        Ok(ComputerInfo {
            ready: true,
            protocol_version: 1,
            detail: None,
        })
    }

    fn list_dir(&self, path: &str) -> Result<Vec<WorkspaceEntry>, ComputerError> {
        let response = guest_call(self, "list_dir", serde_json::json!({ "path": path }))?;
        let raw = response.stdout.unwrap_or_default();
        let parsed: Vec<GuestWorkspaceEntry> =
            serde_json::from_str(&raw).map_err(|e| ComputerError::ExecutionFailed(e.to_string()))?;
        Ok(parsed
            .into_iter()
            .map(|e| WorkspaceEntry {
                name: e.name,
                path: e.path,
                is_dir: e.kind == "directory",
            })
            .collect())
    }

    fn read_file(&self, path: &str) -> Result<Vec<u8>, ComputerError> {
        let response = guest_call(self, "read_file", serde_json::json!({ "path": path }))?;
        Ok(response.stdout.unwrap_or_default().into_bytes())
    }

    fn write_file(&self, path: &str, data: &[u8]) -> Result<(), ComputerError> {
        let content = String::from_utf8_lossy(data);
        guest_call(
            self,
            "write_file",
            serde_json::json!({ "path": path, "content": content }),
        )?;
        Ok(())
    }

    fn exec(&self, command: &str) -> Result<ExecResult, ComputerError> {
        let response = guest_call(self, "exec", serde_json::json!({ "command": command }))?;
        Ok(ExecResult {
            ok: response.ok,
            stdout: response.stdout.unwrap_or_default(),
            stderr: response.stderr.unwrap_or_default(),
            exit_code: response.exit_code.unwrap_or(-1),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GuestWorkspaceEntry {
    name: String,
    path: String,
    kind: String,
}

fn guest_call(
    computer: &LocalMacComputer,
    method: &str,
    params: serde_json::Value,
) -> Result<crate::vm::GuestResponse, ComputerError> {
    let response = computer
        .vm
        .guest_request(GuestRequest {
            id: uuid::Uuid::new_v4().to_string(),
            method: method.into(),
            params,
        })
        .map_err(|e| ComputerError::ExecutionFailed(e))?;

    if !response.ok {
        let message = response
            .error
            .unwrap_or_else(|| "guest request failed".into());
        if message.contains("sandbox") || message.contains("blocked") {
            return Err(ComputerError::SandboxRejected(message));
        }
        return Err(ComputerError::ExecutionFailed(message));
    }

    Ok(response)
}
