//! Manual / developer probe: real ChatGPT subscription turn using direct Elsewhere MCP tools.

use std::time::Duration;

use agent_core::{AgentComputer, AllowAllApprovalGate, FakeAgentComputer, ToolRunContext};
use computer_mcp::{ComputerMcpServer, MCP_BEARER_ENV_VAR};
use serde_json::Value;

use crate::assistant_accumulator::CodexAssistantAccumulator;
use crate::client::CodexAppServerClient;
use crate::compat::ensure_codex_mcp_tool_exposure_supported;
use crate::error::CodexProviderError;
use crate::process::{codex_version, which_codex_executable, CodexProcessLaunch};
use crate::protocol::rpc::IncomingMessage;
use crate::protocol::{
    account_auth_metadata, item_from_notification, notification_thread_turn, parse_turn_completed,
    require_chatgpt_account, turn_error_message, CodexAccountKind, ElsewhereThreadConfig,
    MCP_SERVER_NAME,
};

pub const DIRECT_TOOL_PROBE_PROMPT: &str = "Write /workspace/direct-tool-proof.txt containing exactly:\n\ndirect Elsewhere MCP works\n\nThen read the file and reply with exactly what it contains.";

pub const DIRECT_TOOL_PROBE_EXPECTED_FILE: &str = "/workspace/direct-tool-proof.txt";
pub const DIRECT_TOOL_PROBE_EXPECTED_CONTENT: &str = "direct Elsewhere MCP works";

const TURN_TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Default)]
struct TurnProbeState {
    assistant: CodexAssistantAccumulator,
    last_turn_completed: Option<Value>,
    mcp_write_started: bool,
    mcp_write_completed: bool,
    mcp_read_started: bool,
    mcp_read_completed: bool,
    host_tool_violation: bool,
}

#[derive(Debug)]
pub struct McpTurnProbeResult {
    pub codex_version: String,
    pub auth_type: String,
    pub plan_type: Option<String>,
    pub model: String,
    pub mcp_tools_discovered: Vec<String>,
    pub assistant_text: String,
    pub mcp_write_started: bool,
    pub mcp_write_completed: bool,
    pub mcp_read_started: bool,
    pub mcp_read_completed: bool,
    pub host_tool_violation: bool,
}

pub async fn run_mcp_turn_probe(model: &str) -> Result<McpTurnProbeResult, CodexProviderError> {
    ensure_codex_mcp_tool_exposure_supported()?;

    let executable = which_codex_executable()?;
    let version = codex_version(&executable)?;

    let computer = std::sync::Arc::new(FakeAgentComputer::new());
    let run = ToolRunContext {
        run_id: "probe".into(),
        request_id: "req".into(),
        owner_id: "local".into(),
        bot_id: "bot".into(),
        computer_id: "comp".into(),
        tool_invocation_id: None,
    };
    let mcp = ComputerMcpServer::start(
        computer.clone(),
        std::sync::Arc::new(AllowAllApprovalGate),
        run,
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        None,
        None,
        None,
        None,
        None,
        "conv-probe".into(),
    )
    .await
    .map_err(|e| CodexProviderError::RunEngine(e.to_string()))?;

    let cwd = tempfile::tempdir().map_err(|e| CodexProviderError::Config(e.to_string()))?;
    let thread_config = ElsewhereThreadConfig {
        cwd: cwd.path().to_path_buf(),
        mcp_url: mcp.url().to_string(),
        bearer_env_var: MCP_BEARER_ENV_VAR.to_string(),
        model: model.to_string(),
        base_instructions: None,
        developer_instructions: None,
    };

    let launch = CodexProcessLaunch::from_path(executable)
        .subscription_child()
        .with_env(MCP_BEARER_ENV_VAR, mcp.bearer_token());
    let client = CodexAppServerClient::launch(launch).await?;

    let account = client.account().await?;
    let plan_type = require_chatgpt_account(&account).ok();
    let (auth_type, _) = account_auth_metadata(&account);
    let account_label = match &account.account {
        CodexAccountKind::ChatGpt { plan_type, .. } => format!("chatgpt ({plan_type})"),
        CodexAccountKind::ApiKey => "apiKey".into(),
        CodexAccountKind::NotLoggedIn => "not_logged_in".into(),
        CodexAccountKind::Other(kind) => kind.clone(),
    };

    println!("Codex version: {version}");
    println!("Account type: {account_label}");
    println!("Plan type: {}", plan_type.as_deref().unwrap_or("unknown"));
    println!("Model: {model}");

    let thread_id = client.thread_start_elsewhere(&thread_config).await?;
    let tools = client
        .list_mcp_server_tools_named(&thread_id, Some(MCP_SERVER_NAME))
        .await?;
    println!("MCP server discovered: {MCP_SERVER_NAME}");
    println!("Raw tool names discovered: {tools:?}");

    for required in [
        "workspace_list",
        "workspace_read",
        "workspace_write",
        "workspace_exec",
    ] {
        if !tools.iter().any(|name| name == required) {
            let _ = client.shutdown().await;
            mcp.shutdown().await;
            return Err(CodexProviderError::RunEngine(format!(
                "missing required MCP tool before turn: {required}"
            )));
        }
    }

    let mut notifications = client.notifications();

    let turn_id = client
        .turn_start(
            &thread_id,
            DIRECT_TOOL_PROBE_PROMPT,
            Duration::from_secs(60),
        )
        .await?;

    let mut state = TurnProbeState::default();
    let deadline = tokio::time::Instant::now() + TURN_TIMEOUT;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            let _ = client.shutdown().await;
            mcp.shutdown().await;
            return Err(CodexProviderError::Timeout("mcp_turn_probe".into()));
        }

        let message = match tokio::time::timeout(remaining, notifications.recv()).await {
            Ok(Ok(msg)) => msg,
            Ok(Err(_)) => continue,
            Err(_) => {
                let _ = client.shutdown().await;
                mcp.shutdown().await;
                return Err(CodexProviderError::Timeout("mcp_turn_probe".into()));
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
                    handle_probe_item_started(item, &mut state);
                }
            }
            "item/completed" => {
                if let Some(item) = item_from_notification(&params) {
                    handle_probe_item_completed(item, &mut state);
                }
            }
            "item/agentMessage/delta" => {
                state.assistant.on_agent_message_delta(&params);
            }
            "turn/completed" => {
                state.last_turn_completed = Some(params.clone());
                let (_, _, status) = parse_turn_completed(&params)?;
                if status != "completed" {
                    let message = turn_error_message(&params)
                        .unwrap_or_else(|| format!("turn ended with status={status}"));
                    let _ = client.shutdown().await;
                    mcp.shutdown().await;
                    return Err(CodexProviderError::RunEngine(message));
                }
                drain_late_notifications(
                    &mut notifications,
                    &thread_id,
                    &turn_id,
                    &mut state,
                    Duration::from_secs(5),
                )
                .await;
                println!("turn completed");
                break;
            }
            _ => {}
        }
    }

    if !state.mcp_write_started || !state.mcp_write_completed {
        let _ = client.shutdown().await;
        mcp.shutdown().await;
        return Err(CodexProviderError::RunEngine(
            "missing real workspace_write MCP tool call".into(),
        ));
    }
    if !state.mcp_read_started || !state.mcp_read_completed {
        let _ = client.shutdown().await;
        mcp.shutdown().await;
        return Err(CodexProviderError::RunEngine(
            "missing real workspace_read MCP tool call".into(),
        ));
    }
    if state.host_tool_violation {
        let _ = client.shutdown().await;
        mcp.shutdown().await;
        return Err(CodexProviderError::RunEngine(
            "host commandExecution/fileChange used instead of Elsewhere MCP".into(),
        ));
    }

    let file_bytes = computer
        .read_file(DIRECT_TOOL_PROBE_EXPECTED_FILE)
        .await
        .map_err(|e| CodexProviderError::RunEngine(e.to_string()))?;
    let file_text = String::from_utf8_lossy(&file_bytes);
    if file_text.trim() != DIRECT_TOOL_PROBE_EXPECTED_CONTENT {
        let _ = client.shutdown().await;
        mcp.shutdown().await;
        return Err(CodexProviderError::RunEngine(format!(
            "FakeAgentComputer file content mismatch: got {:?}",
            file_text.trim()
        )));
    }

    if !state
        .assistant
        .canonical_success(state.last_turn_completed.as_ref())
        .contains(DIRECT_TOOL_PROBE_EXPECTED_CONTENT)
    {
        let assistant_text = state
            .assistant
            .canonical_success(state.last_turn_completed.as_ref());
        let _ = client.shutdown().await;
        mcp.shutdown().await;
        return Err(CodexProviderError::RunEngine(format!(
            "assistant text missing expected content; got {:?}",
            assistant_text.trim()
        )));
    }

    let assistant_text = state
        .assistant
        .canonical_success(state.last_turn_completed.as_ref());
    let result = McpTurnProbeResult {
        codex_version: version,
        auth_type,
        plan_type,
        model: model.to_string(),
        mcp_tools_discovered: tools,
        assistant_text,
        mcp_write_started: state.mcp_write_started,
        mcp_write_completed: state.mcp_write_completed,
        mcp_read_started: state.mcp_read_started,
        mcp_read_completed: state.mcp_read_completed,
        host_tool_violation: state.host_tool_violation,
    };

    let _ = client.shutdown().await;
    mcp.shutdown().await;
    Ok(result)
}

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

fn handle_probe_item_started(item: &Value, state: &mut TurnProbeState) {
    let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if is_disallowed_host_item(item_type) {
        state.host_tool_violation = true;
        println!("disallowed host tool started: {item_type}");
        return;
    }
    if item_type == "agentMessage" {
        state.assistant.on_item_started(item);
        return;
    }
    if item_type != "mcpToolCall" {
        return;
    }
    let server = item.get("server").and_then(|v| v.as_str()).unwrap_or("");
    let tool = item.get("tool").and_then(|v| v.as_str()).unwrap_or("");
    if server != MCP_SERVER_NAME {
        return;
    }
    match tool {
        "workspace_write" => {
            state.mcp_write_started = true;
            println!("MCP started: elsewhere/workspace_write");
        }
        "workspace_read" => {
            state.mcp_read_started = true;
            println!("MCP started: elsewhere/workspace_read");
        }
        _ => {}
    }
}

fn handle_probe_item_completed(item: &Value, state: &mut TurnProbeState) {
    let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if is_disallowed_host_item(item_type) {
        state.host_tool_violation = true;
        return;
    }
    if item_type == "agentMessage" {
        state.assistant.on_agent_message_completed(item);
        return;
    }
    if item_type != "mcpToolCall" {
        return;
    }
    let server = item.get("server").and_then(|v| v.as_str()).unwrap_or("");
    let tool = item.get("tool").and_then(|v| v.as_str()).unwrap_or("");
    if server != MCP_SERVER_NAME {
        return;
    }
    let status = item.get("status").and_then(|v| v.as_str()).unwrap_or("");
    let ok = status == "completed" || item.get("success").and_then(|v| v.as_bool()) == Some(true);
    if !ok {
        let message = item
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("unknown MCP tool error");
        println!("MCP failed: elsewhere/{tool}: {message}");
        return;
    }
    match tool {
        "workspace_write" => {
            state.mcp_write_completed = true;
            println!("MCP completed: elsewhere/workspace_write");
        }
        "workspace_read" => {
            state.mcp_read_completed = true;
            println!("MCP completed: elsewhere/workspace_read");
        }
        _ => {}
    }
}

async fn drain_late_notifications(
    notifications: &mut tokio::sync::broadcast::Receiver<IncomingMessage>,
    thread_id: &str,
    turn_id: &str,
    state: &mut TurnProbeState,
    grace: Duration,
) {
    let deadline = tokio::time::Instant::now() + grace;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        let message = match tokio::time::timeout(remaining, notifications.recv()).await {
            Ok(Ok(msg)) => msg,
            _ => break,
        };
        let IncomingMessage::Notification { method, params } = message else {
            continue;
        };
        if !notification_matches_active(thread_id, turn_id, &params) {
            continue;
        }
        match method.as_str() {
            "item/started" => {
                if let Some(item) = item_from_notification(&params) {
                    handle_probe_item_started(item, state);
                }
            }
            "item/completed" => {
                if let Some(item) = item_from_notification(&params) {
                    handle_probe_item_completed(item, state);
                }
            }
            "item/agentMessage/delta" => {
                state.assistant.on_agent_message_delta(&params);
            }
            _ => {}
        }
    }
}

fn is_disallowed_host_item(item_type: &str) -> bool {
    matches!(
        item_type,
        "commandExecution" | "fileChange" | "applyPatch" | "shell" | "localShell"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn agent_message_completion_replaces_delta_accumulation() {
        let mut state = TurnProbeState::default();
        state
            .assistant
            .on_agent_message_delta(&json!({ "itemId": "msg-1", "delta": "partial del" }));

        let item = json!({
            "type": "agentMessage",
            "id": "msg-1",
            "text": "direct Elsewhere MCP works"
        });
        handle_probe_item_completed(&item, &mut state);

        assert_eq!(
            state.assistant.canonical_success(None),
            "direct Elsewhere MCP works"
        );
    }
}
