use agent_core::{
    filter_workspace_listing, AgentComputer, ComputerError, ComputerInfo, ExecResult,
    WorkspaceEntry, WorkspaceRevisionCounter, workspace_tool_mutation,
};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::warn;

use crate::browser::{
    ensure_browser_guest, invoke_browser_daemon, read_browser_preview_cache, BrowserPreviewCache,
};
use crate::client::{SpriteClient, SpriteClientConfig};
use crate::policy::NetworkPolicyConfig;
use crate::types::{Checkpoint, SpriteError};

#[derive(Debug, Clone)]
pub struct SpriteComputerConfig {
    pub base_url: String,
    pub token: String,
    pub sprite_name: String,
    pub workspace_root: String,
    pub request_timeout: Duration,
    pub auto_create: bool,
    pub network_policy: NetworkPolicyConfig,
    pub exec_timeout: Duration,
    pub browser_enabled: bool,
    pub browser_exec_timeout: Duration,
}

impl SpriteComputerConfig {
    pub fn into_client_config(self) -> SpriteClientConfig {
        SpriteClientConfig {
            base_url: self.base_url,
            token: self.token,
            sprite_name: self.sprite_name,
            workspace_root: self.workspace_root,
            request_timeout: self.request_timeout,
            auto_create: self.auto_create,
            network_policy: self.network_policy,
        }
    }
}

pub struct SpriteComputer {
    client: Arc<SpriteClient>,
    workspace_root: String,
    exec_timeout: Duration,
    browser_enabled: bool,
    browser_exec_timeout: Duration,
    network_policy: NetworkPolicyConfig,
    /// Serializes `workspace_exec` and browser work so egress cannot overlap shell exec.
    execution_gate: Mutex<()>,
    /// One-time workspace directory bootstrap (cold sprites).
    workspace_bootstrap: Mutex<()>,
    /// Cold sprites may exist before `/workspace` is created on disk.
    workspace_materialized: AtomicBool,
    revision_counter: WorkspaceRevisionCounter,
}

impl SpriteComputer {
    pub fn new(config: SpriteComputerConfig) -> Result<Self, SpriteError> {
        let exec_timeout = config.exec_timeout;
        let browser_exec_timeout = config.browser_exec_timeout;
        let browser_enabled = config.browser_enabled;
        let network_policy = config.network_policy.clone();
        let workspace_root = config.workspace_root.clone();
        let client = Arc::new(SpriteClient::new(config.into_client_config())?);
        Ok(Self {
            client,
            workspace_root,
            exec_timeout,
            browser_enabled,
            browser_exec_timeout,
            network_policy,
            execution_gate: Mutex::new(()),
            workspace_bootstrap: Mutex::new(()),
            workspace_materialized: AtomicBool::new(false),
            revision_counter: WorkspaceRevisionCounter::default(),
        })
    }

    /// Sprites can be provisioned while the workspace directory still does not exist.
    async fn materialize_workspace(&self) -> Result<(), ComputerError> {
        if self.workspace_materialized.load(Ordering::Acquire) {
            return Ok(());
        }
        let _guard = self.workspace_bootstrap.lock().await;
        if self.workspace_materialized.load(Ordering::Acquire) {
            return Ok(());
        }

        self.client
            .ensure_sprite()
            .await
            .map_err(map_sprite_error)?;

        let root = self.workspace_root.trim_end_matches('/');
        let marker = format!("{root}/.elsewhere-bootstrap");

        if self
            .client
            .fs_write(&marker, b"1", true)
            .await
            .is_ok()
        {
            self.workspace_materialized.store(true, Ordering::Release);
            return Ok(());
        }

        let _gate = self.execution_gate.lock().await;
        for workdir in ["/home/sprite", root] {
            let (_, _, code) = self
                .client
                .exec_http(&format!("mkdir -p {root}"), workdir, self.exec_timeout)
                .await
                .map_err(map_sprite_error)?;
            if code != 0 {
                continue;
            }
            if self
                .client
                .fs_write(&marker, b"1", true)
                .await
                .is_ok()
            {
                self.workspace_materialized.store(true, Ordering::Release);
                return Ok(());
            }
        }

        Err(ComputerError::GuestUnavailable(
            "failed to initialize workspace directory".into(),
        ))
    }

    pub fn client(&self) -> &SpriteClient {
        &self.client
    }

    pub async fn create_checkpoint(
        &self,
        comment: Option<&str>,
    ) -> Result<Checkpoint, SpriteError> {
        self.client.create_checkpoint(comment).await
    }

    pub async fn list_checkpoints(&self) -> Result<Vec<Checkpoint>, SpriteError> {
        self.client.list_checkpoints().await
    }

    pub async fn restore_checkpoint(&self, checkpoint_id: &str) -> Result<(), SpriteError> {
        self.client.restore_checkpoint(checkpoint_id).await
    }

    /// Observability-only read of the guest preview cache (no browser lifecycle work).
    pub async fn read_browser_preview_cache(&self) -> Result<BrowserPreviewCache, ComputerError> {
        read_browser_preview_cache(&self.client).await
    }

    fn normalize_path(&self, path: &str) -> Result<String, ComputerError> {
        let root = self.workspace_root.trim_end_matches('/');
        let normalized = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("{root}/{path}")
        };
        if normalized != root && !normalized.starts_with(&format!("{root}/")) {
            return Err(ComputerError::SandboxRejected(format!(
                "path outside workspace: {path}"
            )));
        }
        Ok(normalized)
    }

    fn absolutize_workspace_path(&self, path: &str) -> String {
        let root = self.workspace_root.trim_end_matches('/');
        if path.starts_with('/') {
            return path.to_string();
        }
        if path == "." || path.is_empty() {
            return root.to_string();
        }
        format!("{root}/{path}")
    }
}

#[async_trait]
impl AgentComputer for SpriteComputer {
    fn invalidate_cached_readiness(&self) {
        // SpriteComputer has no in-memory readiness cache; ReadinessCachedComputer wraps this.
    }

    async fn ensure_ready(&self) -> Result<ComputerInfo, ComputerError> {
        self.materialize_workspace().await?;
        let info = self
            .client
            .get_sprite()
            .await
            .map_err(map_sprite_error)?;

        self.client
            .fs_write(
                &format!("{}/.elsewhere-ready", self.workspace_root),
                b"1",
                true,
            )
            .await
            .map_err(map_sprite_error)?;

        {
            let _gate = self.execution_gate.lock().await;
            let (_, _, code) = self
                .client
                .exec_http("pwd", &self.workspace_root, self.exec_timeout)
                .await
                .map_err(map_sprite_error)?;
            if code != 0 {
                return Err(ComputerError::GuestUnavailable(
                    "workspace exec probe failed".into(),
                ));
            }
        }

        Ok(ComputerInfo {
            ready: true,
            protocol_version: 1,
            detail: Some(format!("sprite {} ({})", info.name, info.status)),
        })
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<WorkspaceEntry>, ComputerError> {
        let path = self.normalize_path(path)?;
        let response = self.client.fs_list(&path).await.map_err(|err| {
            if let SpriteError::NotFound = err {
                return ComputerError::ExecutionFailed(format!("directory not found: {path}"));
            }
            if let SpriteError::Provider { status, message } = &err {
                if *status == 400 {
                    warn!(
                        sprite_name = %self.client.sprite_name(),
                        path = %path,
                        provider_message = %message,
                        "sprites fs/list rejected"
                    );
                }
            }
            map_sprite_error(err)
        })?;
        let entries: Vec<WorkspaceEntry> = response
            .entries
            .into_iter()
            .map(|entry| WorkspaceEntry {
                name: entry.name,
                path: self.absolutize_workspace_path(&entry.path),
                is_dir: entry.is_dir || entry.r#type.as_deref() == Some("directory"),
            })
            .collect();
        Ok(filter_workspace_listing(entries))
    }

    fn workspace_revision(&self) -> u64 {
        self.revision_counter.get()
    }

    fn record_workspace_mutation(&self, tool_name: &str, result: &serde_json::Value) {
        if workspace_tool_mutation(tool_name, result) {
            self.revision_counter.bump();
        }
    }

    async fn read_file(&self, path: &str) -> Result<Vec<u8>, ComputerError> {
        let path = self.normalize_path(path)?;
        self.materialize_workspace().await?;
        self.client
            .fs_read(&path)
            .await
            .map_err(|err| match err {
                SpriteError::NotFound => ComputerError::ExecutionFailed(format!("file not found: {path}")),
                other => map_sprite_error(other),
            })
    }

    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), ComputerError> {
        let path = self.normalize_path(path)?;
        self.materialize_workspace().await?;
        self.client
            .fs_write(&path, data, true)
            .await
            .map_err(map_sprite_error)
    }

    async fn exec(&self, command: &str) -> Result<ExecResult, ComputerError> {
        let _gate = self.execution_gate.lock().await;
        let (stdout, stderr, exit_code) = self
            .client
            .exec_http(command, &self.workspace_root, self.exec_timeout)
            .await
            .map_err(map_sprite_error)?;
        Ok(ExecResult {
            ok: exit_code == 0,
            stdout,
            stderr,
            exit_code,
        })
    }

    async fn browser_invoke(&self, action: &str, args: &Value) -> Result<Value, ComputerError> {
        if !self.browser_enabled {
            return Err(ComputerError::SandboxRejected(
                "browser automation is disabled for this computer".into(),
            ));
        }

        self.materialize_workspace().await?;

        let _gate = self.execution_gate.lock().await;

        ensure_browser_guest(
            &self.client,
            &self.network_policy,
            self.browser_exec_timeout,
        )
        .await?;

        let mut request = args.clone();
        if let Some(obj) = request.as_object_mut() {
            obj.insert("action".into(), json!(action));
        }
        let payload = serde_json::to_string(&request).map_err(|e| {
            ComputerError::MalformedArguments(format!("browser request JSON: {e}"))
        })?;

        let require_egress = browser_action_requires_network_egress(action);
        let stdout = invoke_browser_daemon(
            &self.client,
            &self.network_policy,
            &payload,
            self.browser_exec_timeout,
            require_egress,
        )
        .await?;

        let line = stdout.lines().last().unwrap_or(stdout.trim());
        serde_json::from_str(line).map_err(|e| {
            ComputerError::ExecutionFailed(format!("browser daemon returned invalid JSON: {e}"))
        })
    }
}

/// Browser actions that may trigger navigation, subresource loads, or form submits.
pub fn browser_action_requires_network_egress(action: &str) -> bool {
    matches!(action, "navigate" | "download" | "click" | "type")
}

fn map_sprite_error(err: SpriteError) -> ComputerError {
    match err {
        SpriteError::NotFound => ComputerError::NotProvisioned,
        SpriteError::Timeout => ComputerError::ExecutionFailed("command timed out".into()),
        SpriteError::Config(msg) => ComputerError::MalformedArguments(msg),
        SpriteError::Network(msg) => ComputerError::GuestUnavailable(msg),
        SpriteError::NotReady(msg) => ComputerError::GuestUnavailable(msg),
        SpriteError::MalformedResponse(msg) => ComputerError::ExecutionFailed(msg),
        SpriteError::Provider { status, message } if status == 403 || status == 400 => {
            ComputerError::SandboxRejected(message)
        }
        SpriteError::Provider { message, .. } => ComputerError::GuestUnavailable(message),
    }
}

#[cfg(test)]
mod tests {
    use super::browser_action_requires_network_egress;

    #[test]
    fn interactive_browser_actions_require_temporary_egress() {
        for action in ["navigate", "download", "click", "type"] {
            assert!(browser_action_requires_network_egress(action));
        }
        for action in ["snapshot", "screenshot", "preview", "health"] {
            assert!(!browser_action_requires_network_egress(action));
        }
    }
}
