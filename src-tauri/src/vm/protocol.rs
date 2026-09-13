use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VmLifecycleState {
    NotCreated,
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VmInfo {
    pub state: VmLifecycleState,
    pub host_arch: String,
    pub guest_arch: String,
    pub cpu_count: u32,
    pub memory_mib: u32,
    pub disk_path: String,
    pub disk_bytes: u64,
    pub config_path: String,
    pub guest_bridge_ready: bool,
    pub message: Option<String>,
    pub created: bool,
    pub console_log_path: String,
    pub vmm_binary_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestRequest {
    pub id: String,
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestResponse {
    pub id: String,
    pub ok: bool,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
    pub protocol_version: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VmConfigFile {
    pub id: String,
    pub host_arch: String,
    pub guest_arch: String,
    pub cpu_count: u32,
    pub memory_mib: u32,
    pub kernel_path: String,
    pub initrd_path: Option<String>,
    pub disk_path: String,
    pub kernel_command_line: String,
    pub guest_agent_port: u32,
    pub console_log_path: String,
}
