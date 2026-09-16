use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;

use crate::error::CodexProviderError;
use crate::protocol::rpc::{
    error_envelope, notification_envelope, parse_incoming_line, request_envelope,
    response_envelope, IncomingMessage, RequestId,
};

pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct CodexProcessLaunch {
    pub executable: PathBuf,
    pub config_overrides: Vec<String>,
    pub env: HashMap<String, String>,
    /// When true, strip `OPENAI_API_KEY` from the child environment (subscription proof path).
    pub strip_openai_api_key: bool,
}

impl CodexProcessLaunch {
    pub fn from_path(executable: PathBuf) -> Self {
        Self {
            executable,
            config_overrides: Vec::new(),
            env: HashMap::new(),
            strip_openai_api_key: false,
        }
    }

    pub fn subscription_child(mut self) -> Self {
        self.strip_openai_api_key = true;
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Keep account state in a host-owned profile, never in the agent computer.
    pub fn with_profile(mut self, home: &Path) -> Self {
        self.env
            .insert("CODEX_HOME".into(), home.to_string_lossy().into_owned());
        self.config_overrides
            .push("cli_auth_credentials_store=\"file\"".into());
        self
    }
}

pub struct ManagedCodexProcess {
    child: Mutex<Option<Child>>,
    writer: mpsc::Sender<String>,
    _stdout_task: JoinHandle<()>,
    _stderr_task: JoinHandle<()>,
    notifications: broadcast::Sender<IncomingMessage>,
    pending: ArcPending,
    next_id: Mutex<i64>,
}

type PendingMap =
    HashMap<RequestId, oneshot::Sender<Result<serde_json::Value, CodexProviderError>>>;
type ArcPending = std::sync::Arc<Mutex<PendingMap>>;

impl ManagedCodexProcess {
    pub async fn spawn(launch: CodexProcessLaunch) -> Result<Self, CodexProviderError> {
        let mut command = Command::new(&launch.executable);
        command
            .arg("app-server")
            .arg("--stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command.kill_on_drop(true);
        for override_arg in &launch.config_overrides {
            command.arg("-c").arg(override_arg);
        }
        for (key, value) in &launch.env {
            command.env(key, value);
        }
        if launch.strip_openai_api_key {
            command.env_remove("OPENAI_API_KEY");
        }

        let mut child = command
            .spawn()
            .map_err(|e| CodexProviderError::Process(format!("spawn codex app-server: {e}")))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| CodexProviderError::Process("app-server missing stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| CodexProviderError::Process("app-server missing stdout".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| CodexProviderError::Process("app-server missing stderr".into()))?;

        let mut process = Self::from_async_io(stdin, stdout, stderr).await?;
        *process.child.get_mut() = Some(child);
        Ok(process)
    }

    pub async fn from_async_io(
        stdin: impl tokio::io::AsyncWrite + Send + Unpin + 'static,
        stdout: impl tokio::io::AsyncRead + Send + Unpin + 'static,
        stderr: impl tokio::io::AsyncRead + Send + Unpin + 'static,
    ) -> Result<Self, CodexProviderError> {
        let (writer_tx, mut writer_rx) = mpsc::channel::<String>(64);
        let pending: ArcPending = std::sync::Arc::new(Mutex::new(HashMap::new()));
        let (notification_tx, _) = broadcast::channel(256);

        let mut stdin = stdin;
        tokio::spawn(async move {
            while let Some(payload) = writer_rx.recv().await {
                if stdin.write_all(payload.as_bytes()).await.is_err() {
                    break;
                }
                if stdin.write_all(b"\n").await.is_err() {
                    break;
                }
                let _ = stdin.flush().await;
            }
        });

        let stdout_pending = pending.clone();
        let stdout_notifications = notification_tx.clone();
        let writer_for_stdout = writer_tx.clone();
        let stdout_task = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        if let Ok(Some(message)) = parse_incoming_line(&line) {
                            route_incoming(
                                message,
                                &stdout_pending,
                                &stdout_notifications,
                                &writer_for_stdout,
                            )
                            .await;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let stderr_task = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            loop {
                line.clear();
                if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                    break;
                }
                if !line.trim().is_empty() {
                    tracing::debug!(target: "codex_app_server_stderr", "{}", line.trim());
                }
            }
        });

        Ok(Self {
            child: Mutex::new(None),
            writer: writer_tx,
            _stdout_task: stdout_task,
            _stderr_task: stderr_task,
            notifications: notification_tx,
            pending,
            next_id: Mutex::new(1),
        })
    }

    pub fn subscribe_notifications(&self) -> broadcast::Receiver<IncomingMessage> {
        self.notifications.subscribe()
    }

    pub async fn send_raw(&self, payload: String) -> Result<(), CodexProviderError> {
        self.writer
            .send(payload)
            .await
            .map_err(|_| CodexProviderError::Process("app-server stdin closed".into()))
    }

    pub async fn request(
        &self,
        method: &str,
        params: serde_json::Value,
        timeout: Duration,
    ) -> Result<serde_json::Value, CodexProviderError> {
        let id = {
            let mut next = self.next_id.lock().await;
            let current = *next;
            *next += 1;
            current
        };
        let request_id = RequestId::Number(id);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(request_id.clone(), tx);

        let envelope = request_envelope(id, method, params);
        self.send_raw(envelope.to_string()).await?;

        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(CodexProviderError::Process(
                "app-server response channel closed".into(),
            )),
            Err(_) => {
                self.pending.lock().await.remove(&request_id);
                Err(CodexProviderError::Timeout(method.to_string()))
            }
        }
    }

    pub async fn notify(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<(), CodexProviderError> {
        self.send_raw(notification_envelope(method, params).to_string())
            .await
    }

    pub async fn shutdown(&self) -> Result<(), CodexProviderError> {
        if let Some(mut child) = self.child.lock().await.take() {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
        Ok(())
    }
}

async fn route_incoming(
    message: IncomingMessage,
    pending: &ArcPending,
    notifications: &broadcast::Sender<IncomingMessage>,
    writer: &mpsc::Sender<String>,
) {
    match message {
        IncomingMessage::Response { id, result } => {
            if let Some(tx) = pending.lock().await.remove(&id) {
                let _ = tx.send(Ok(result));
            }
        }
        IncomingMessage::Error { id, error } => {
            if let Some(tx) = pending.lock().await.remove(&id) {
                let _ = tx.send(Err(CodexProviderError::Protocol(format!(
                    "json-rpc {}: {}",
                    error.code, error.message
                ))));
            }
        }
        IncomingMessage::Notification { .. } => {
            let _ = notifications.send(message);
        }
        IncomingMessage::ServerRequest { id, method, params } => {
            let _ = notifications.send(IncomingMessage::ServerRequest {
                id: id.clone(),
                method: method.clone(),
                params,
            });
            if let Some(result) = auto_deny_server_request(&method) {
                let payload = response_envelope(&id, result).to_string();
                let _ = writer.send(payload).await;
            } else {
                let payload =
                    error_envelope(&id, -32601, "method not supported by Elsewhere client")
                        .to_string();
                let _ = writer.send(payload).await;
            }
        }
    }
}

fn auto_deny_server_request(method: &str) -> Option<serde_json::Value> {
    match method {
        "execCommandApproval" | "item/commandExecution/requestApproval" => {
            Some(serde_json::json!({ "decision": "deny" }))
        }
        "item/fileChange/requestApproval" | "applyPatchApproval" => {
            Some(serde_json::json!({ "decision": "deny" }))
        }
        "mcpServer/elicitation/request" => Some(serde_json::json!({ "action": "decline" })),
        _ => None,
    }
}

pub fn which_codex_executable() -> Result<PathBuf, CodexProviderError> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join("codex");
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(CodexProviderError::CodexNotInstalled)
}

pub fn codex_version(executable: &Path) -> Result<String, CodexProviderError> {
    let output = std::process::Command::new(executable)
        .arg("--version")
        .output()
        .map_err(|e| CodexProviderError::Process(e.to_string()))?;
    if !output.status.success() {
        return Err(CodexProviderError::Process("codex --version failed".into()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
