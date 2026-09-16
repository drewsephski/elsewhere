//! Nested tool-less Codex helper on the parent app-server process.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use agent_core::{bound_subagent_result, SubagentError, SubagentTurn, SUBAGENT_TURN_TIMEOUT_SECS};
use async_trait::async_trait;

use crate::client::CodexAppServerClient;
use crate::error::CodexProviderError;
use crate::protocol::ToollessThreadConfig;
use crate::toolless_turn::{run_toolless_turn_on_client, ToollessTurnOptions};

pub struct NestedCodexTurn {
    client: Arc<CodexAppServerClient>,
    cwd: PathBuf,
    model: String,
    cancel: Arc<AtomicBool>,
}

impl NestedCodexTurn {
    pub fn new(
        client: Arc<CodexAppServerClient>,
        cwd: PathBuf,
        model: impl Into<String>,
        cancel: Arc<AtomicBool>,
    ) -> Self {
        Self {
            client,
            cwd,
            model: model.into(),
            cancel,
        }
    }
}

#[async_trait]
impl SubagentTurn for NestedCodexTurn {
    async fn run_toolless(
        &self,
        model: &str,
        developer_instructions: &str,
        user_prompt: &str,
        _cancel: &AtomicBool,
    ) -> Result<String, SubagentError> {
        let config = ToollessThreadConfig {
            cwd: self.cwd.clone(),
            model: if model.trim().is_empty() {
                self.model.clone()
            } else {
                model.to_string()
            },
            developer_instructions: Some(developer_instructions.to_string()),
        };
        let text = run_toolless_turn_on_client(
            self.client.as_ref(),
            &config,
            user_prompt,
            ToollessTurnOptions {
                cancel: Some(self.cancel.as_ref()),
                extra_cancel: Some(_cancel),
                timeout: Duration::from_secs(SUBAGENT_TURN_TIMEOUT_SECS),
                shutdown_client: false,
            },
        )
        .await
        .map_err(map_codex_error)?;
        Ok(bound_subagent_result(&text))
    }
}

fn map_codex_error(err: CodexProviderError) -> SubagentError {
    match err {
        CodexProviderError::Timeout(m) => {
            SubagentError::Internal(format!("subagent timed out: {m}"))
        }
        CodexProviderError::RunEngine(m) if m == "cancelled" => SubagentError::Cancelled,
        other => SubagentError::Internal(other.to_string()),
    }
}
