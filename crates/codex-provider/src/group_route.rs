//! One-shot group routing decision via Codex subscription (tool-less thread).

use std::time::Duration;

use crate::assistant_accumulator::CodexAssistantAccumulator;
use crate::client::CodexAppServerClient;
use crate::error::CodexProviderError;
use crate::process::{which_codex_executable, CodexProcessLaunch};
use crate::protocol::{
    item_from_notification, notification_thread_turn, parse_turn_completed, turn_error_message,
    ToollessThreadConfig,
};
use crate::protocol::rpc::IncomingMessage;

const ROUTE_TURN_TIMEOUT: Duration = Duration::from_secs(120);

fn notification_matches_active(thread_id: &str, turn_id: &str, params: &serde_json::Value) -> bool {
    let (msg_thread, msg_turn) = notification_thread_turn(params);
    msg_thread.as_deref() == Some(thread_id) && msg_turn.as_deref() == Some(turn_id)
}

pub async fn run_codex_group_route_decision(
    executable: Option<std::path::PathBuf>,
    profile: Option<std::path::PathBuf>,
    model: &str,
    developer_instructions: &str,
    user_prompt: &str,
) -> Result<String, CodexProviderError> {
    let executable = executable
        .or_else(|| which_codex_executable().ok())
        .ok_or(CodexProviderError::CodexNotInstalled)?;

    let cwd = tempfile::tempdir().map_err(|e| CodexProviderError::Config(e.to_string()))?;
    let thread_config = ToollessThreadConfig {
        cwd: cwd.path().to_path_buf(),
        model: model.to_string(),
        developer_instructions: Some(developer_instructions.to_string()),
    };

    let mut launch = CodexProcessLaunch::from_path(executable).subscription_child();
    if let Some(profile) = profile {
        launch = launch.with_profile(&profile);
    }

    let client = CodexAppServerClient::launch(launch).await?;
    let thread_id = client.thread_start_toolless(&thread_config).await?;
    let mut notifications = client.notifications();
    let turn_id = client
        .turn_start(&thread_id, user_prompt, Duration::from_secs(60))
        .await?;

    let mut assistant = CodexAssistantAccumulator::default();
    let mut last_turn_completed: Option<serde_json::Value> = None;
    let deadline = tokio::time::Instant::now() + ROUTE_TURN_TIMEOUT;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            let _ = client.shutdown().await;
            return Err(CodexProviderError::Timeout("group_route_turn".into()));
        }

        let message = match tokio::time::timeout(remaining, notifications.recv()).await {
            Ok(Ok(msg)) => msg,
            Ok(Err(_)) => continue,
            Err(_) => {
                let _ = client.shutdown().await;
                return Err(CodexProviderError::Timeout("group_route_turn".into()));
            }
        };

        let IncomingMessage::Notification { method, params } = message else {
            continue;
        };
        if !notification_matches_active(&thread_id, &turn_id, &params) {
            continue;
        }

        match method.as_str() {
            "item/started" => {
                if let Some(item) = item_from_notification(&params) {
                    assistant.on_item_started(item);
                }
            }
            "item/completed" => {
                if let Some(item) = item_from_notification(&params) {
                    assistant.on_agent_message_completed(item);
                }
            }
            "item/agentMessage/delta" => assistant.on_agent_message_delta(&params),
            "turn/completed" => {
                last_turn_completed = Some(params.clone());
                let (_, _, status) = parse_turn_completed(&params)?;
                if status != "completed" {
                    let message = turn_error_message(&params)
                        .unwrap_or_else(|| format!("turn ended with status={status}"));
                    let _ = client.shutdown().await;
                    return Err(CodexProviderError::RunEngine(message));
                }
                break;
            }
            _ => {}
        }
    }

    let text = assistant.canonical_success(last_turn_completed.as_ref());
    let _ = client.shutdown().await;
    Ok(text)
}
