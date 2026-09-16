use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use super::protocol::GuestResponse;

#[derive(Debug, Serialize)]
struct ControlRequest {
    cmd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<String>,
    #[serde(rename = "timeoutSeconds", skip_serializing_if = "Option::is_none")]
    timeout_seconds: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct VmRuntimeStatus {
    pub phase: String,
    pub message: Option<String>,
    #[serde(rename = "guestBridgeReady")]
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
    pub binary_path: PathBuf,
}

impl VmmProcess {
    pub fn spawn(
        vmm_binary: &Path,
        config_path: &Path,
        socket_path: &Path,
        log_path: &Path,
    ) -> Result<Self, String> {
        verify_vmm_binary(vmm_binary)?;

        if socket_path.exists() {
            let _ = std::fs::remove_file(socket_path);
        }

        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .map_err(|e| e.to_string())?;

        tracing::info!(vmm = %vmm_binary.display(), "spawning gptbot-vmm");

        let mut child = Command::new(vmm_binary)
            .arg("serve")
            .arg("--config")
            .arg(config_path)
            .arg("--socket")
            .arg(socket_path)
            .stdout(Stdio::from(
                log_file.try_clone().map_err(|e| e.to_string())?,
            ))
            .stderr(Stdio::from(log_file))
            .spawn()
            .map_err(|e| {
                format!(
                    "failed to spawn gptbot-vmm at {}: {e}",
                    vmm_binary.display()
                )
            })?;

        wait_for_socket(socket_path, Duration::from_secs(10))?;
        if let Ok(Some(status)) = child.try_wait() {
            let tail = tail_file(log_path, 4096).unwrap_or_default();
            return Err(format!(
                "gptbot-vmm exited before serving (status: {status}). Log tail:\n{tail}"
            ));
        }

        Ok(Self {
            child,
            binary_path: vmm_binary.to_path_buf(),
        })
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
            // Do not connect here: a probe connection is accepted by the VMM and can
            // interfere with the real control request on the single-threaded server.
            std::thread::sleep(Duration::from_millis(50));
            return Ok(());
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

    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| format!("set read timeout: {e}"))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|e| format!("set write timeout: {e}"))?;

    let request = ControlRequest {
        cmd: cmd.to_string(),
        payload,
        timeout_seconds: Some(timeout.as_secs_f64()),
    };
    let line = serde_json::to_string(&request).map_err(|e| e.to_string())?;
    stream
        .write_all(line.as_bytes())
        .map_err(|e| map_io_timeout(e, timeout, "write VMM control request"))?;
    stream
        .write_all(b"\n")
        .map_err(|e| map_io_timeout(e, timeout, "write VMM control newline"))?;
    stream
        .shutdown(Shutdown::Write)
        .map_err(|e| format!("shutdown VMM control socket write half: {e}"))?;

    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    reader
        .read_line(&mut response_line)
        .map_err(|e| map_io_timeout(e, timeout, "read VMM control response"))?;

    if response_line.trim().is_empty() {
        return Err(format!(
            "empty VMM response for `{cmd}` (gptbot-vmm may have exited). socket={}",
            socket_path.display()
        ));
    }

    let trimmed = response_line.trim();
    if trimmed.is_empty() {
        return Err(
            "invalid VMM response: empty body (gptbot-vmm may have exited during the request)"
                .into(),
        );
    }
    serde_json::from_str(trimmed)
        .map_err(|e| format!("invalid VMM response: {e} (body: {trimmed})"))
}

fn map_io_timeout(err: std::io::Error, timeout: Duration, op: &str) -> String {
    if err.kind() == std::io::ErrorKind::WouldBlock || err.kind() == std::io::ErrorKind::TimedOut {
        format!("{op} timed out after {}s", timeout.as_secs_f64())
    } else {
        format!("{op}: {err}")
    }
}

pub fn parse_guest_response(result: &str) -> Result<GuestResponse, String> {
    serde_json::from_str(result).map_err(|e| format!("invalid guest JSON: {e}"))
}

/// Resolve the `gptbot-vmm` helper that will actually be executed (must be signed with virtualization entitlement).
pub fn locate_vmm_binary() -> Result<PathBuf, String> {
    let candidates: Vec<PathBuf> = {
        let mut list = Vec::new();
        if let Ok(path) = std::env::var("GPTBOT_VMM_PATH") {
            list.push(PathBuf::from(path));
        }
        list.push(PathBuf::from(env!("OUT_DIR")).join("gptbot-vmm"));
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        list.push(manifest.join("../macos/gptbot-vmm/.build/release/gptbot-vmm"));
        list
    };

    let mut errors: Vec<String> = Vec::new();
    for candidate in candidates {
        if !candidate.exists() {
            continue;
        }
        match verify_vmm_binary(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(err) => errors.push(format!("{}: {}", candidate.display(), err)),
        }
    }

    Err(format!(
        "no usable signed gptbot-vmm found. Build with `cargo build` (signs OUT_DIR copy) or export GPTBOT_VMM_PATH to a codesigned binary with com.apple.security.virtualization. Details:\n{}",
        errors.join("\n")
    ))
}

pub fn verify_vmm_binary(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err("binary does not exist".into());
    }

    let output = Command::new("codesign")
        .args(["-dvvv", "--entitlements", ":-"])
        .arg(path)
        .output()
        .map_err(|e| format!("codesign failed to run: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("not codesigned or invalid signature: {stderr}"));
    }

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !combined.contains("com.apple.security.virtualization") {
        return Err(
            "missing com.apple.security.virtualization entitlement (re-sign with src-tauri/entitlements.plist and --generate-entitlement-der)"
                .into(),
        );
    }

    Ok(())
}

pub fn tail_file(path: &Path, max_bytes: usize) -> Option<String> {
    let data = std::fs::read(path).ok()?;
    if data.is_empty() {
        return None;
    }
    let start = data.len().saturating_sub(max_bytes);
    String::from_utf8_lossy(&data[start..]).to_string().into()
}
