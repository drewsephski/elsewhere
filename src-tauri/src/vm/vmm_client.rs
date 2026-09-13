use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use super::protocol::GuestResponse;

#[derive(Debug, Serialize)]
struct ControlRequest {
    cmd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout_seconds: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct VmRuntimeStatus {
    pub phase: String,
    pub message: Option<String>,
    pub guest_bridge_ready: bool,
}

#[derive(Debug, Deserialize)]
pub struct ControlResponse {
    pub ok: bool,
    pub error: Option<String>,
    pub status: Option<VmRuntimeStatus>,
    pub result: Option<String>,
}

pub struct VmmProcess {
    child: Child,
}

impl VmmProcess {
    pub fn spawn(
        vmm_binary: &Path,
        config_path: &Path,
        socket_path: &Path,
        log_path: &Path,
    ) -> Result<Self, String> {
        if socket_path.exists() {
            std::fs::remove_file(socket_path).map_err(|e| e.to_string())?;
        }

        let log_file = std::fs::File::create(log_path).map_err(|e| e.to_string())?;

        let child = Command::new(vmm_binary)
            .arg("serve")
            .arg("--config")
            .arg(config_path)
            .arg("--socket")
            .arg(socket_path)
            .stdout(Stdio::from(log_file.try_clone().map_err(|e| e.to_string())?))
            .stderr(Stdio::from(log_file))
            .spawn()
            .map_err(|e| format!("failed to spawn gptbot-vmm: {e}"))?;

        wait_for_socket(socket_path, Duration::from_secs(10))?;

        Ok(Self { child })
    }

    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn wait_for_socket(socket_path: &Path, timeout: Duration) -> Result<(), String> {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if socket_path.exists() {
            if UnixStream::connect(socket_path).is_ok() {
                return Ok(());
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err("VMM control socket did not become ready".into())
}

pub fn control_request(
    socket_path: &Path,
    cmd: &str,
    payload: Option<String>,
    timeout: Duration,
) -> Result<ControlResponse, String> {
    let mut stream = UnixStream::connect(socket_path)
        .map_err(|e| format!("connect to VMM control socket: {e}"))?;

    let request = ControlRequest {
        cmd: cmd.to_string(),
        payload,
        timeout_seconds: Some(timeout.as_secs_f64()),
    };
    let line = serde_json::to_string(&request).map_err(|e| e.to_string())?;
    stream
        .write_all(line.as_bytes())
        .map_err(|e| e.to_string())?;
    stream.write_all(b"\n").map_err(|e| e.to_string())?;

    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    reader
        .read_line(&mut response_line)
        .map_err(|e| e.to_string())?;

    serde_json::from_str(response_line.trim()).map_err(|e| format!("invalid VMM response: {e}"))
}

pub fn parse_guest_response(result: &str) -> Result<GuestResponse, String> {
    serde_json::from_str(result).map_err(|e| format!("invalid guest JSON: {e}"))
}

pub fn locate_vmm_binary() -> Result<std::path::PathBuf, String> {
    if let Ok(path) = std::env::var("GPTBOT_VMM_PATH") {
        let p = std::path::PathBuf::from(path);
        if p.exists() {
            return Ok(p);
        }
    }

    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dev_path = manifest
        .join("../macos/gptbot-vmm/.build/release/gptbot-vmm");
    if dev_path.exists() {
        return Ok(dev_path);
    }

    let out_path = std::path::PathBuf::from(env!("OUT_DIR")).join("gptbot-vmm");
    if out_path.exists() {
        return Ok(out_path);
    }

    Err(
        "gptbot-vmm binary not found (build with swift or set GPTBOT_VMM_PATH)".into(),
    )
}
