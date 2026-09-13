//! Minimal GPT Bot guest agent — listens on AF_VSOCK for newline-delimited JSON RPC.

mod sandbox;

use std::fs;
#[cfg(target_os = "linux")]
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};

#[cfg(target_os = "linux")]
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};

use serde::{Deserialize, Serialize};

const GUEST_AGENT_PORT: u32 = 1024;
const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
struct GuestRequest {
    id: String,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GuestResponse {
    id: String,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    protocol_version: u32,
}

fn main() {
    if let Err(err) = run_listener() {
        eprintln!("gptbot-guest-agent fatal: {err}");
        std::process::exit(1);
    }
}

fn run_listener() -> Result<(), String> {
    fs::create_dir_all(sandbox::WORKSPACE_ROOT).map_err(|e| e.to_string())?;

    #[cfg(not(target_os = "linux"))]
    {
        return vsock_listen(GUEST_AGENT_PORT).map(|_| ());
    }

    #[cfg(target_os = "linux")]
    {
        let listener = vsock_listen(GUEST_AGENT_PORT)?;
        eprintln!(
            "gptbot-guest-agent listening on vsock port {} (protocol v{})",
            GUEST_AGENT_PORT,
            PROTOCOL_VERSION
        );

        for stream in listener.incoming() {
            match stream {
                Ok(conn) => {
                    if let Err(err) = handle_connection(conn) {
                        eprintln!("connection error: {err}");
                    }
                }
                Err(err) => return Err(format!("accept failed: {err}")),
            }
        }

        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn handle_connection(stream: VsockStream) -> Result<(), String> {
    let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
    let mut writer = stream;
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .map_err(|e| e.to_string())?;
    let trimmed = request_line.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    let request: GuestRequest = serde_json::from_str(trimmed)
        .map_err(|e| format!("invalid JSON request: {e}"))?;
    let response = dispatch(&request);
    let json = serde_json::to_string(&response).map_err(|e| e.to_string())?;
    writer
        .write_all(json.as_bytes())
        .map_err(|e| e.to_string())?;
    writer.write_all(b"\n").map_err(|e| e.to_string())?;
    Ok(())
}

fn dispatch(request: &GuestRequest) -> GuestResponse {
    let base = |ok: bool| GuestResponse {
        id: request.id.clone(),
        ok,
        stdout: None,
        stderr: None,
        exit_code: None,
        error: None,
        protocol_version: PROTOCOL_VERSION,
    };

    match request.method.as_str() {
        "ping" | "health" => {
            let mut r = base(true);
            r.stdout = Some("pong".into());
            r.exit_code = Some(0);
            r
        }
        "exec" => {
            let command = request
                .params
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match sandbox::validate_exec_command(command) {
                Ok(()) => run_shell_command(&request.id, command),
                Err(err) => {
                    let mut r = base(false);
                    r.error = Some(err);
                    r
                }
            }
        }
        "write_file" => {
            let path = request.params.get("path").and_then(|v| v.as_str());
            let content = request.params.get("content").and_then(|v| v.as_str());
            match (path, content) {
                (Some(path), Some(content)) => match sandbox::write_workspace_file(path, content) {
                    Ok(()) => {
                        let mut r = base(true);
                        r.exit_code = Some(0);
                        r
                    }
                    Err(err) => {
                        let mut r = base(false);
                        r.error = Some(err);
                        r
                    }
                },
                _ => {
                    let mut r = base(false);
                    r.error = Some("write_file requires path and content".into());
                    r
                }
            }
        }
        "list_dir" => {
            let path = request
                .params
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or(sandbox::WORKSPACE_ROOT);
            match sandbox::list_workspace_dir(path) {
                Ok(entries) => match serde_json::to_string(&entries) {
                    Ok(json) => {
                        let mut r = base(true);
                        r.stdout = Some(json);
                        r.exit_code = Some(0);
                        r
                    }
                    Err(err) => {
                        let mut r = base(false);
                        r.error = Some(err.to_string());
                        r
                    }
                },
                Err(err) => {
                    let mut r = base(false);
                    r.error = Some(err);
                    r
                }
            }
        }
        "read_file" => {
            let path = request
                .params
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if path.is_empty() {
                let mut r = base(false);
                r.error = Some("read_file requires path".into());
                return r;
            }
            match sandbox::read_workspace_file(path) {
                Ok(text) => {
                    let mut r = base(true);
                    r.stdout = Some(text);
                    r.exit_code = Some(0);
                    r
                }
                Err(err) => {
                    let mut r = base(false);
                    r.error = Some(err);
                    r
                }
            }
        }
        other => {
            let mut r = base(false);
            r.error = Some(format!("unknown method: {other}"));
            r
        }
    }
}

fn run_shell_command(request_id: &str, command: &str) -> GuestResponse {
    let output = Command::new("/bin/sh")
        .arg("-c")
        .arg(command)
        .current_dir(sandbox::workspace_dir_for_exec())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match output {
        Ok(out) => GuestResponse {
            id: request_id.to_string(),
            ok: out.status.success(),
            stdout: Some(String::from_utf8_lossy(&out.stdout).to_string()),
            stderr: Some(String::from_utf8_lossy(&out.stderr).to_string()),
            exit_code: Some(out.status.code().unwrap_or(-1)),
            error: None,
            protocol_version: PROTOCOL_VERSION,
        },
        Err(err) => GuestResponse {
            id: request_id.to_string(),
            ok: false,
            stdout: None,
            stderr: None,
            exit_code: None,
            error: Some(err.to_string()),
            protocol_version: PROTOCOL_VERSION,
        },
    }
}

/// Minimal vsock listener using libc (Linux guest only).
fn vsock_listen(port: u32) -> Result<VsockListener, String> {
    #[cfg(target_os = "linux")]
    {
        use std::mem;
        const AF_VSOCK: i32 = 40;
        const SOCK_STREAM: i32 = 1;

        let fd = unsafe { libc::socket(AF_VSOCK, SOCK_STREAM, 0) };
        if fd < 0 {
            return Err(format!("socket failed: {}", std::io::Error::last_os_error()));
        }

        #[repr(C)]
        struct sockaddr_vm {
            svm_family: u16,
            svm_reserved1: u16,
            svm_port: u32,
            svm_cid: u32,
            svm_zero: [u8; 4],
        }

        let addr = sockaddr_vm {
            svm_family: AF_VSOCK as u16,
            svm_reserved1: 0,
            svm_port: port,
            svm_cid: u32::MAX, // VMADDR_CID_ANY
            svm_zero: [0; 4],
        };

        let bind_res = unsafe {
            libc::bind(
                fd,
                &addr as *const _ as *const libc::sockaddr,
                mem::size_of::<sockaddr_vm>() as libc::socklen_t,
            )
        };
        if bind_res != 0 {
            unsafe { libc::close(fd) };
            return Err(format!("bind failed: {}", std::io::Error::last_os_error()));
        }

        if unsafe { libc::listen(fd, 8) } != 0 {
            unsafe { libc::close(fd) };
            return Err(format!("listen failed: {}", std::io::Error::last_os_error()));
        }

        Ok(VsockListener { fd })
    }

    #[cfg(not(target_os = "linux"))]
    {
        Err("gptbot-guest-agent only runs on Linux".into())
    }
}

#[cfg(not(target_os = "linux"))]
struct VsockListener;

#[cfg(target_os = "linux")]
struct VsockListener {
    fd: RawFd,
}

#[cfg(not(target_os = "linux"))]
impl VsockListener {
    fn incoming(&self) -> std::iter::Empty<Result<(), String>> {
        std::iter::empty()
    }
}

#[cfg(target_os = "linux")]
impl VsockListener {
    fn incoming(&self) -> VsockIncoming {
        VsockIncoming { fd: self.fd }
    }
}

#[cfg(target_os = "linux")]
impl Drop for VsockListener {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

#[cfg(target_os = "linux")]
struct VsockIncoming {
    fd: RawFd,
}

#[cfg(target_os = "linux")]
impl Iterator for VsockIncoming {
    type Item = Result<VsockStream, String>;

    fn next(&mut self) -> Option<Self::Item> {
        let client = unsafe { libc::accept(self.fd, std::ptr::null_mut(), std::ptr::null_mut()) };
        if client < 0 {
            return Some(Err(format!(
                "accept failed: {}",
                std::io::Error::last_os_error()
            )));
        }
        Some(Ok(unsafe { VsockStream::from_raw_fd(client) }))
    }
}

/// Stream I/O over an accepted AF_VSOCK connection (not AF_UNIX).
#[cfg(target_os = "linux")]
struct VsockStream {
    fd: OwnedFd,
}

#[cfg(target_os = "linux")]
impl VsockStream {
    fn try_clone(&self) -> Result<Self, String> {
        let dup_fd = unsafe { libc::dup(self.fd.as_raw_fd()) };
        if dup_fd < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(unsafe { VsockStream::from_raw_fd(dup_fd) })
    }
}

#[cfg(target_os = "linux")]
impl FromRawFd for VsockStream {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        VsockStream {
            fd: OwnedFd::from_raw_fd(fd),
        }
    }
}

#[cfg(target_os = "linux")]
impl Read for VsockStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = unsafe {
            libc::read(
                self.fd.as_raw_fd(),
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
            )
        };
        if n < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(n as usize)
        }
    }
}

#[cfg(target_os = "linux")]
impl Write for VsockStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = unsafe {
            libc::write(
                self.fd.as_raw_fd(),
                buf.as_ptr() as *const libc::c_void,
                buf.len(),
            )
        };
        if n < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(n as usize)
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
