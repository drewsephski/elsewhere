//! One-shot group routing decision via Codex subscription (tool-less thread).

use std::time::Duration;

use serde_json::Value;
use tokio::sync::broadcast::error::RecvError;

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
const TURN_START_TIMEOUT: Duration = Duration::from_secs(60);
/// Grace period for item notifications that may arrive after `turn/completed`.
const LATE_NOTIFICATION_GRACE: Duration = Duration::from_secs(5);

#[derive(Debug, Default)]
struct RouteTurnState {
    assistant: CodexAssistantAccumulator,
    last_turn_completed: Option<Value>,
}

/// Matches `run_engine` / `mcp_turn_probe`: ignore only when IDs are present and disagree.
fn notification_matches_active(thread_id: &str, turn_id: &str, params: &Value) -> bool {
    let (msg_thread, msg_turn) = notification_thread_turn(params);
    if let Some(tid) = msg_thread {
        if tid != thread_id {
            return false;
        }
    }
    if let Some(turn) = msg_turn {
        if turn != turn_id {
            return false;
        }
    }
    true
}

fn apply_assistant_notification(method: &str, params: &Value, state: &mut RouteTurnState) {
    match method {
        "item/started" => {
            if let Some(item) = item_from_notification(params) {
                state.assistant.on_item_started(item);
            }
        }
        "item/completed" => {
            if let Some(item) = item_from_notification(params) {
                state.assistant.on_agent_message_completed(item);
            }
        }
        "item/agentMessage/delta" => state.assistant.on_agent_message_delta(params),
        _ => {}
    }
}

async fn drain_late_assistant_notifications(
    notifications: &mut tokio::sync::broadcast::Receiver<IncomingMessage>,
    thread_id: &str,
    turn_id: &str,
    state: &mut RouteTurnState,
) {
    let deadline = tokio::time::Instant::now() + LATE_NOTIFICATION_GRACE;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        let message = match tokio::time::timeout(remaining, notifications.recv()).await {
            Ok(Ok(msg)) => msg,
            Ok(Err(RecvError::Lagged(_))) => continue,
            _ => break,
        };
        let IncomingMessage::Notification { method, params } = message else {
            continue;
        };
        if !notification_matches_active(thread_id, turn_id, &params) {
            continue;
        }
        apply_assistant_notification(&method, &params, state);
    }
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
    thread_config.validate()?;

    let mut launch = CodexProcessLaunch::from_path(executable).subscription_child();
    if let Some(profile) = profile {
        launch = launch.with_profile(&profile);
    }

    let client = CodexAppServerClient::launch(launch).await?;
    let thread_id = client.thread_start_toolless(&thread_config).await?;
    let mut notifications = client.notifications();
    let turn_id = client
        .turn_start(&thread_id, user_prompt, TURN_START_TIMEOUT)
        .await?;

    let mut state = RouteTurnState::default();
    let deadline = tokio::time::Instant::now() + ROUTE_TURN_TIMEOUT;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            let _ = client.shutdown().await;
            return Err(CodexProviderError::Timeout("group_route_turn".into()));
        }

        let message = match tokio::time::timeout(remaining, notifications.recv()).await {
            Ok(Ok(msg)) => msg,
            Ok(Err(RecvError::Lagged(_))) => continue,
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
            "item/started" | "item/completed" | "item/agentMessage/delta" => {
                apply_assistant_notification(&method, &params, &mut state);
            }
            "turn/completed" => {
                state.last_turn_completed = Some(params.clone());
                let (_, _, status) = parse_turn_completed(&params)?;
                if status != "completed" {
                    let message = turn_error_message(&params)
                        .unwrap_or_else(|| format!("turn ended with status={status}"));
                    let _ = client.shutdown().await;
                    return Err(CodexProviderError::RunEngine(message));
                }
                drain_late_assistant_notifications(
                    &mut notifications,
                    &thread_id,
                    &turn_id,
                    &mut state,
                )
                .await;
                break;
            }
            _ => {}
        }
    }

    let text = state
        .assistant
        .canonical_success(state.last_turn_completed.as_ref());
    let _ = client.shutdown().await;

    if text.trim().is_empty() {
        return Err(CodexProviderError::RunEngine(
            "group route turn produced empty assistant text".into(),
        ));
    }

    Ok(text)
}
