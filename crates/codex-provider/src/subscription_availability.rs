use std::path::PathBuf;
use std::time::Duration;

use crate::client::CodexAppServerClient;
use crate::error::CodexProviderError;
use crate::process::{which_codex_executable, CodexProcessLaunch};
use crate::protocol::{require_chatgpt_account, CodexAccountKind};

const PROBE_STARTUP_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexSubscriptionAvailability {
    Available {
        plan_type: Option<String>,
    },
    NotInstalled,
    NotAuthenticated,
    NotChatGpt,
    Unavailable(String),
}

/// Preflight ChatGPT subscription path without starting a run or reading auth files.
pub async fn probe_codex_subscription_availability(
    executable: Option<PathBuf>,
) -> CodexSubscriptionAvailability {
    let executable = match executable.or_else(|| which_codex_executable().ok()) {
        Some(path) => path,
        None => return CodexSubscriptionAvailability::NotInstalled,
    };

    let launch = CodexProcessLaunch::from_path(executable).subscription_child();

    let client = match tokio::time::timeout(PROBE_STARTUP_TIMEOUT, CodexAppServerClient::launch(launch))
        .await
    {
        Ok(Ok(client)) => client,
        Ok(Err(CodexProviderError::CodexNotInstalled)) => {
            return CodexSubscriptionAvailability::NotInstalled;
        }
        Ok(Err(err)) => return CodexSubscriptionAvailability::Unavailable(err.to_string()),
        Err(_) => {
            return CodexSubscriptionAvailability::Unavailable(
                "Codex app-server startup timed out during availability probe".into(),
            );
        }
    };

    let account = match client.account().await {
        Ok(state) => state,
        Err(err) => {
            let _ = client.shutdown().await;
            return map_account_probe_error(err);
        }
    };

    let plan_type = match require_chatgpt_account(&account) {
        Ok(plan) => Some(plan),
        Err(CodexProviderError::Account(msg)) if msg == "codex_not_authenticated" => {
            let _ = client.shutdown().await;
            return CodexSubscriptionAvailability::NotAuthenticated;
        }
        Err(CodexProviderError::Account(msg)) if msg == "codex_not_chatgpt" => {
            let _ = client.shutdown().await;
            return CodexSubscriptionAvailability::NotChatGpt;
        }
        Err(err) => {
            let _ = client.shutdown().await;
            return CodexSubscriptionAvailability::Unavailable(err.to_string());
        }
    };

    let _ = client.shutdown().await;

    CodexSubscriptionAvailability::Available { plan_type }
}

fn map_account_probe_error(err: CodexProviderError) -> CodexSubscriptionAvailability {
    match err {
        CodexProviderError::Account(msg) if msg == "codex_not_authenticated" => {
            CodexSubscriptionAvailability::NotAuthenticated
        }
        CodexProviderError::Account(msg) if msg == "codex_not_chatgpt" => {
            CodexSubscriptionAvailability::NotChatGpt
        }
        CodexProviderError::Account(msg) => CodexSubscriptionAvailability::Unavailable(msg),
        other => CodexSubscriptionAvailability::Unavailable(other.to_string()),
    }
}

#[cfg(any(test, feature = "test-utils"))]
pub async fn probe_codex_subscription_availability_on_client(
    client: &CodexAppServerClient,
) -> CodexSubscriptionAvailability {
    let account = match client.account().await {
        Ok(state) => state,
        Err(err) => return map_account_probe_error(err),
    };

    match &account.account {
        CodexAccountKind::NotLoggedIn => CodexSubscriptionAvailability::NotAuthenticated,
        CodexAccountKind::ApiKey => CodexSubscriptionAvailability::NotChatGpt,
        CodexAccountKind::ChatGpt { plan_type, .. } => CodexSubscriptionAvailability::Available {
            plan_type: Some(plan_type.clone()),
        },
        CodexAccountKind::Other(kind) => CodexSubscriptionAvailability::Unavailable(format!(
            "codex_unsupported_account:{kind}"
        )),
    }
}
