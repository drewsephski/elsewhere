use crate::vm::{GuestRequest, VirtualMachineManager};
use agent_core::{AgentComputer, ComputerError, ComputerInfo, ExecResult, WorkspaceEntry};
use async_trait::async_trait;
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

#[async_trait]
impl AgentComputer for LocalMacComputer {
    async fn ensure_ready(&self) -> Result<ComputerInfo, ComputerError> {
        let vm = self.vm.clone();
        tokio::task::spawn_blocking(move || ensure_ready_sync(&vm))
            .await
            .map_err(|_| ComputerError::ExecutionFailed("VM readiness task failed".into()))?
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<WorkspaceEntry>, ComputerError> {
        let vm = self.vm.clone();
        let path = path.to_string();
        tokio::task::spawn_blocking(move || list_dir_sync(&vm, &path))
            .await
            .map_err(|_| ComputerError::ExecutionFailed("list_dir task failed".into()))?
    }

    async fn read_file(&self, path: &str) -> Result<Vec<u8>, ComputerError> {
        let vm = self.vm.clone();
        let path = path.to_string();
        tokio::task::spawn_blocking(move || read_file_sync(&vm, &path))
            .await
            .map_err(|_| ComputerError::ExecutionFailed("read_file task failed".into()))?
    }

    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), ComputerError> {
        let vm = self.vm.clone();
        let path = path.to_string();
        let data = data.to_vec();
        tokio::task::spawn_blocking(move || write_file_sync(&vm, &path, &data))
            .await
            .map_err(|_| ComputerError::ExecutionFailed("write_file task failed".into()))?
    }

    async fn exec(&self, command: &str) -> Result<ExecResult, ComputerError> {
        let vm = self.vm.clone();
        let command = command.to_string();
        tokio::task::spawn_blocking(move || exec_sync(&vm, &command))
            .await
            .map_err(|_| ComputerError::ExecutionFailed("exec task failed".into()))?
    }
}

fn ensure_ready_sync(vm: &VirtualMachineManager) -> Result<ComputerInfo, ComputerError> {
    if !vm.layout().vm_config_path().exists() {
        return Err(ComputerError::NotProvisioned);
    }
    vm.ensure_guest_ready(GUEST_READY_TIMEOUT).map_err(|err| {
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

fn list_dir_sync(
    vm: &VirtualMachineManager,
    path: &str,
) -> Result<Vec<WorkspaceEntry>, ComputerError> {
    let response = guest_call(vm, "list_dir", serde_json::json!({ "path": path }))?;
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

fn read_file_sync(vm: &VirtualMachineManager, path: &str) -> Result<Vec<u8>, ComputerError> {
    let response = guest_call(
        vm,
        "read_file",
        serde_json::json!({ "path": path, "encoding": "base64" }),
    )?;
    let encoded = response.stdout.unwrap_or_default();
    agent_core::decode_bytes(&encoded).map_err(ComputerError::ExecutionFailed)
}

fn write_file_sync(
    vm: &VirtualMachineManager,
    path: &str,
    data: &[u8],
) -> Result<(), ComputerError> {
    guest_call(
        vm,
        "write_file",
        serde_json::json!({
            "path": path,
            "content": agent_core::encode_bytes(data),
            "encoding": "base64",
        }),
    )?;
    Ok(())
}

fn exec_sync(vm: &VirtualMachineManager, command: &str) -> Result<ExecResult, ComputerError> {
    let response = guest_call(vm, "exec", serde_json::json!({ "command": command }))?;
    Ok(ExecResult {
        ok: response.ok,
        stdout: response.stdout.unwrap_or_default(),
        stderr: response.stderr.unwrap_or_default(),
        exit_code: response.exit_code.unwrap_or(-1),
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GuestWorkspaceEntry {
    name: String,
    path: String,
    kind: String,
}

fn guest_call(
    vm: &VirtualMachineManager,
    method: &str,
    params: serde_json::Value,
) -> Result<crate::vm::GuestResponse, ComputerError> {
    let response = vm
        .guest_request(GuestRequest {
            id: uuid::Uuid::new_v4().to_string(),
            method: method.into(),
            params,
        })
        .map_err(ComputerError::ExecutionFailed)?;

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
