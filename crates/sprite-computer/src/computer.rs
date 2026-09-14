use agent_core::{
    AgentComputer, ComputerError, ComputerInfo, ExecResult, WorkspaceEntry,
};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

use crate::browser::{
    ensure_browser_guest, map_browser_exec_error, BROWSER_CLI, BROWSER_DIR, BROWSER_REQUEST,
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
}

impl SpriteComputer {
    pub fn new(config: SpriteComputerConfig) -> Result<Self, SpriteError> {
        let exec_timeout = config.exec_timeout;
        let browser_exec_timeout = config.browser_exec_timeout;
        let browser_enabled = config.browser_enabled;
        let workspace_root = config.workspace_root.clone();
        let client = Arc::new(SpriteClient::new(config.into_client_config())?);
        Ok(Self {
            client,
            workspace_root,
            exec_timeout,
            browser_enabled,
            browser_exec_timeout,
        })
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
}

#[async_trait]
impl AgentComputer for SpriteComputer {
    async fn ensure_ready(&self) -> Result<ComputerInfo, ComputerError> {
        let info = self
            .client
            .ensure_sprite()
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

        if self.browser_enabled {
            ensure_browser_guest(&self.client, self.browser_exec_timeout).await?;
        }

        Ok(ComputerInfo {
            ready: true,
            protocol_version: 1,
            detail: Some(format!("sprite {} ({})", info.name, info.status)),
        })
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<WorkspaceEntry>, ComputerError> {
        let path = self.normalize_path(path)?;
        let response = self.client.fs_list(&path).await.map_err(map_sprite_error)?;
        Ok(response
            .entries
            .into_iter()
            .map(|entry| WorkspaceEntry {
                name: entry.name,
                path: entry.path,
                is_dir: entry.is_dir || entry.r#type.as_deref() == Some("directory"),
            })
            .collect())
    }

    async fn read_file(&self, path: &str) -> Result<Vec<u8>, ComputerError> {
        let path = self.normalize_path(path)?;
        self.client
            .fs_read(&path)
            .await
            .map_err(map_sprite_error)
    }

    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), ComputerError> {
        let path = self.normalize_path(path)?;
        self.client
            .fs_write(&path, data, true)
            .await
            .map_err(map_sprite_error)
    }

    async fn exec(&self, command: &str) -> Result<ExecResult, ComputerError> {
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

        ensure_browser_guest(&self.client, self.browser_exec_timeout).await?;

        let mut request = args.clone();
        if let Some(obj) = request.as_object_mut() {
            obj.insert("action".into(), json!(action));
        }
        let payload = serde_json::to_string(&request).map_err(|e| {
            ComputerError::MalformedArguments(format!("browser request JSON: {e}"))
        })?;
        self.client
            .fs_write(BROWSER_REQUEST, payload.as_bytes(), true)
            .await
            .map_err(map_sprite_error)?;

        let command = format!(
            "export PLAYWRIGHT_BROWSERS_PATH={BROWSER_DIR}/browsers; \
             export PATH={BROWSER_DIR}/node-runtime/bin:$PATH; \
             node {BROWSER_CLI} --request {BROWSER_REQUEST}"
        );
        let (stdout, stderr, exit_code) = self
            .client
            .exec_http(&command, &self.workspace_root, self.browser_exec_timeout)
            .await
            .map_err(map_sprite_error)?;

        if exit_code != 0 {
            return Err(map_browser_exec_error(&stdout, &stderr, exit_code));
        }

        let line = stdout.lines().last().unwrap_or(stdout.trim());
        serde_json::from_str(line).map_err(|e| {
            ComputerError::ExecutionFailed(format!("browser CLI returned invalid JSON: {e}"))
        })
    }
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
