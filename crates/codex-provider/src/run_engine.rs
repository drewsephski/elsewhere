use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use agent_core::{
    AgentLoopContext, RunEngine, RunEngineKind, RuntimeError, SharedRunDeps, ToolRunContext,
    DEFAULT_MODEL,
};
use computer_mcp::{ComputerMcpServer, MCP_BEARER_ENV_VAR};

use crate::assistant_accumulator::CodexAssistantAccumulator;
use crate::client::CodexAppServerClient;
use crate::compat::ensure_codex_mcp_tool_exposure_supported;
use crate::error::CodexProviderError;
use crate::process::{which_codex_executable, CodexProcessLaunch};
use crate::protocol::rpc::IncomingMessage as RpcMessage;
use crate::protocol::{
    account_auth_metadata, item_from_notification, notification_thread_turn, parse_turn_completed,
    require_chatgpt_account, turn_error_message, ElsewhereThreadConfig, MCP_SERVER_NAME,
};
use crate::run_input::user_text_from_run_input;
use crate::run_persistence::{
    emit_run_started, fail_run, finalize_cancelled, finalize_interrupted, finalize_success,
    persist_event,
};
use agent_core::MessageStatus;

const EXECUTION_POLICY: &str = "Your computer is the Elsewhere MCP server. Use workspace_list, workspace_read, workspace_write, and workspace_exec for computer operations. Do not attempt to access the host environment. Request approval by invoking the workspace tool: Elsewhere pauses protected operations and shows the user an approval card before dispatch. Do not replace a tool call with a prose approval request or claim that an operation succeeded before its tool result. Respect denied or expired approvals.";

#[derive(Debug, Clone)]
pub struct CodexRunEngineConfig {
    pub executable: Option<PathBuf>,
    pub profile_home: Option<PathBuf>,
    pub startup_timeout: Duration,
    pub thread_start_timeout: Duration,
    pub turn_start_timeout: Duration,
    pub turn_timeout: Duration,
    pub interrupt_grace: Duration,
    pub shutdown_timeout: Duration,
    pub temp_cwd_root: Option<PathBuf>,
    pub model: String,
}

impl Default for CodexRunEngineConfig {
    fn default() -> Self {
        Self {
            executable: None,
            profile_home: None,
            startup_timeout: Duration::from_secs(120),
            thread_start_timeout: Duration::from_secs(120),
            turn_start_timeout: Duration::from_secs(60),
            turn_timeout: Duration::from_secs(15 * 60),
            interrupt_grace: Duration::from_secs(30),
            shutdown_timeout: Duration::from_secs(30),
            temp_cwd_root: None,
            model: DEFAULT_MODEL.to_string(),
        }
    }
}

pub struct CodexRunEngine {
    config: CodexRunEngineConfig,
}

impl CodexRunEngine {
    pub fn new(config: CodexRunEngineConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(CodexRunEngineConfig::default())
    }
}

#[async_trait]
impl RunEngine for CodexRunEngine {
    fn kind(&self) -> RunEngineKind {
        RunEngineKind::CodexSubscription
    }

    async fn run(
        &self,
        ctx: AgentLoopContext,
        shared: SharedRunDeps,
        input: Vec<Value>,
    ) -> Result<(), RuntimeError> {
        self.run_internal(ctx, shared, input, None).await
    }
}

impl CodexRunEngine {
    /// Test hook: use an already-connected fake or harness app-server process.
    #[cfg(any(test, feature = "test-utils"))]
    pub async fn run_with_managed_process(
        &self,
        ctx: AgentLoopContext,
        shared: SharedRunDeps,
        input: Vec<Value>,
        process: crate::process::ManagedCodexProcess,
    ) -> Result<(), RuntimeError> {
        self.run_internal(ctx, shared, input, Some(process)).await
    }

    async fn run_internal(
        &self,
        ctx: AgentLoopContext,
        shared: SharedRunDeps,
        input: Vec<Value>,
        injected_process: Option<crate::process::ManagedCodexProcess>,
    ) -> Result<(), RuntimeError> {
        tracing::info!(
            target: "elsewhere_run_engine",
            engine = "codex_subscription",
            model = %ctx.model,
            request_id = %ctx.request_id,
            "starting Codex subscription run"
        );

        let user_text = user_text_from_run_input(&input)?;

        if let Err(err) = shared.computer.ensure_ready().await {
            let message = err.to_string();
            fail_run(&shared, &ctx, "computer_not_ready", &message, "", 0).await?;
            return Ok(());
        }

        let tool_run = ToolRunContext {
            run_id: shared.run_id.clone(),
            request_id: ctx.request_id.clone(),
            owner_id: shared.owner_id.clone(),
            bot_id: ctx.bot_id.clone(),
            computer_id: shared.computer_id.clone(),
        };
        let mcp = match ComputerMcpServer::start(
            shared.computer.clone(),
            shared.approval_gate.clone(),
            tool_run,
            shared.cancel.clone(),
        )
        .await
        {
            Ok(server) => server,
            Err(err) => {
                fail_run(
                    &shared,
                    &ctx,
                    "mcp_server_start_failed",
                    &err.to_string(),
                    "",
                    0,
                )
                .await?;
                return Ok(());
            }
        };

        let cwd_dir = match &self.config.temp_cwd_root {
            Some(root) => {
                if let Err(err) = std::fs::create_dir_all(root) {
                    mcp.shutdown().await;
                    fail_run(&shared, &ctx, "codex_cwd_failed", &err.to_string(), "", 0).await?;
                    return Ok(());
                }
                match tempfile::tempdir_in(root) {
                    Ok(dir) => dir,
                    Err(err) => {
                        mcp.shutdown().await;
                        fail_run(&shared, &ctx, "codex_cwd_failed", &err.to_string(), "", 0)
                            .await?;
                        return Ok(());
                    }
                }
            }
            None => match tempfile::tempdir() {
                Ok(dir) => dir,
                Err(err) => {
                    mcp.shutdown().await;
                    fail_run(&shared, &ctx, "codex_cwd_failed", &err.to_string(), "", 0).await?;
                    return Ok(());
                }
            },
        };
        let cwd = cwd_dir
            .path()
            .canonicalize()
            .unwrap_or_else(|_| cwd_dir.path().to_path_buf());

        let using_real_codex = injected_process.is_none();
        let client = if let Some(process) = injected_process {
            match CodexAppServerClient::from_process(process).await {
                Ok(client) => client,
                Err(err) => {
                    mcp.shutdown().await;
                    return map_boot_failure(&shared, &ctx, err).await;
                }
            }
        } else {
            let executable = self
                .config
                .executable
                .clone()
                .or_else(|| which_codex_executable().ok())
                .ok_or_else(|| RuntimeError::Model("codex executable not found".into()))?;
            let mut launch = CodexProcessLaunch::from_path(executable)
                .subscription_child()
                .with_env(MCP_BEARER_ENV_VAR, mcp.bearer_token());
            if let Some(profile) = &self.config.profile_home {
                launch = launch.with_profile(profile);
            }

            match tokio::time::timeout(
                self.config.startup_timeout,
                CodexAppServerClient::launch(launch),
            )
            .await
            {
                Ok(Ok(client)) => client,
                Ok(Err(err)) => {
                    mcp.shutdown().await;
                    return map_boot_failure(&shared, &ctx, err).await;
                }
                Err(_) => {
                    mcp.shutdown().await;
                    fail_run(
                        &shared,
                        &ctx,
                        "codex_startup_timeout",
                        "Codex app-server startup timed out",
                        "",
                        0,
                    )
                    .await?;
                    return Ok(());
                }
            }
        };

        let account = match client.account().await {
            Ok(state) => state,
            Err(err) => {
                let _ = client.shutdown().await;
                mcp.shutdown().await;
                return map_boot_failure(&shared, &ctx, err).await;
            }
        };

        let plan_type = match require_chatgpt_account(&account) {
            Ok(plan) => plan,
            Err(err) => {
                let _ = client.shutdown().await;
                mcp.shutdown().await;
                return map_boot_failure(&shared, &ctx, err).await;
            }
        };

        let (auth_type, _) = account_auth_metadata(&account);

        let instructions = compose_instructions(&ctx.instructions);
        if using_real_codex {
            if let Err(err) = ensure_codex_mcp_tool_exposure_supported() {
                cleanup_run(client, mcp).await;
                return map_boot_failure(&shared, &ctx, err).await;
            }
        }
        let thread_config = ElsewhereThreadConfig {
            cwd,
            mcp_url: mcp.url().to_string(),
            bearer_env_var: MCP_BEARER_ENV_VAR.to_string(),
            model: if ctx.model.is_empty() {
                self.config.model.clone()
            } else {
                ctx.model.clone()
            },
            base_instructions: Some(instructions.base),
            developer_instructions: Some(instructions.developer),
        };

        let thread_id = match tokio::time::timeout(
            self.config.thread_start_timeout,
            client.thread_start_elsewhere(&thread_config),
        )
        .await
        {
            Ok(Ok(id)) => id,
            Ok(Err(err)) => {
                cleanup_run(client, mcp).await;
                return map_boot_failure(&shared, &ctx, err).await;
            }
            Err(_) => {
                cleanup_run(client, mcp).await;
                fail_run(
                    &shared,
                    &ctx,
                    "codex_thread_timeout",
                    "Codex thread/start timed out",
                    "",
                    0,
                )
                .await?;
                return Ok(());
            }
        };

        let tools = match client
            .list_mcp_server_tools_named(&thread_id, Some(MCP_SERVER_NAME))
            .await
        {
            Ok(tools) => tools,
            Err(err) => {
                cleanup_run(client, mcp).await;
                return map_boot_failure(&shared, &ctx, err).await;
            }
        };
        for required in [
            "workspace_list",
            "workspace_read",
            "workspace_write",
            "workspace_exec",
        ] {
            if !tools.iter().any(|name| name == required) {
                cleanup_run(client, mcp).await;
                fail_run(
                    &shared,
                    &ctx,
                    "elsewhere_mcp_missing_tools",
                    &format!("Elsewhere MCP server missing required tool: {required}"),
                    "",
                    0,
                )
                .await?;
                return Ok(());
            }
        }

        let mut notifications = client.notifications();
        let turn_id = match tokio::time::timeout(
            self.config.turn_start_timeout,
            client.turn_start(&thread_id, &user_text, self.config.turn_start_timeout),
        )
        .await
        {
            Ok(Ok(id)) => id,
            Ok(Err(err)) => {
                cleanup_run(client, mcp).await;
                return map_boot_failure(&shared, &ctx, err).await;
            }
            Err(_) => {
                cleanup_run(client, mcp).await;
                fail_run(
                    &shared,
                    &ctx,
                    "codex_turn_start_timeout",
                    "Codex turn/start timed out",
                    "",
                    0,
                )
                .await?;
                return Ok(());
            }
        };

        tracing::info!(
            target: "elsewhere_run_engine",
            engine = "codex_subscription",
            thread_id = %thread_id,
            turn_id = %turn_id,
            "Codex turn started"
        );

        let provider_meta = json!({
            "authType": auth_type,
            "planType": plan_type,
            "threadId": thread_id,
            "turnId": turn_id,
        });
        emit_run_started(&shared, &ctx, &provider_meta).await?;

        let state = TurnRunState {
            thread_id,
            turn_id,
            assistant: CodexAssistantAccumulator::default(),
            last_turn_completed: None,
            step_count: 0,
            tool_calls_seen: HashSet::new(),
            tool_results_seen: HashSet::new(),
            host_tool_violation: false,
        };
        let state = Arc::new(Mutex::new(state));

        let interrupt_sent = Arc::new(AtomicBool::new(false));

        let turn_result = tokio::time::timeout(
            self.config.turn_timeout,
            consume_turn_notifications(
                &client,
                &shared,
                &ctx,
                state.clone(),
                &mut notifications,
                shared.cancel.clone(),
                interrupt_sent.clone(),
                self.config.interrupt_grace,
            ),
        )
        .await;

        let run_outcome = match turn_result {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(err)) => {
                cleanup_run(client, mcp).await;
                return Err(err);
            }
            Err(_) => {
                if !interrupt_sent.swap(true, Ordering::SeqCst) {
                    let _ = client
                        .turn_interrupt(
                            &state.lock().await.thread_id,
                            &state.lock().await.turn_id,
                            Duration::from_secs(30),
                        )
                        .await;
                }
                tokio::time::sleep(self.config.interrupt_grace).await;
                let partial = state.lock().await.assistant.partial_output();
                finalize_interrupted(&shared, &ctx, &partial, "run_timeout").await?;
                cleanup_run(client, mcp).await;
                return Ok(());
            }
        };

        let final_state = state.lock().await;
        let step_count = final_state.step_count;
        let turn_completed = final_state.last_turn_completed.clone();
        let assistant_result = match &run_outcome {
            TurnOutcome::Completed => final_state
                .assistant
                .canonical_success(turn_completed.as_ref()),
            _ => final_state.assistant.partial_output(),
        };
        drop(final_state);

        match run_outcome {
            TurnOutcome::Completed => {
                finalize_success(&shared, &ctx, &assistant_result, step_count).await?;
            }
            TurnOutcome::Failed { code, message } => {
                fail_run(
                    &shared,
                    &ctx,
                    &code,
                    &message,
                    &assistant_result,
                    step_count,
                )
                .await?;
            }
            TurnOutcome::Cancelled => {
                finalize_cancelled(&shared, &ctx, &assistant_result).await?;
            }
            TurnOutcome::Interrupted { code } => {
                finalize_interrupted(&shared, &ctx, &assistant_result, &code).await?;
            }
            TurnOutcome::HostToolViolation => {
                fail_run(
                    &shared,
                    &ctx,
                    "host_tool_violation",
                    "Codex attempted a disallowed host tool during subscription run",
                    &assistant_result,
                    step_count,
                )
                .await?;
            }
            TurnOutcome::ProtocolError(message) => {
                fail_run(
                    &shared,
                    &ctx,
                    "codex_protocol_error",
                    &message,
                    &assistant_result,
                    step_count,
                )
                .await?;
            }
        }

        cleanup_run(client, mcp).await;
        Ok(())
    }
}

enum TurnOutcome {
    Completed,
    Failed { code: String, message: String },
    Cancelled,
    Interrupted { code: String },
    HostToolViolation,
    ProtocolError(String),
}

struct TurnRunState {
    thread_id: String,
    turn_id: String,
    assistant: CodexAssistantAccumulator,
    last_turn_completed: Option<Value>,
    step_count: i64,
    tool_calls_seen: HashSet<String>,
    tool_results_seen: HashSet<String>,
    host_tool_violation: bool,
}

struct InstructionBundle {
    base: String,
    developer: String,
}

fn compose_instructions(user_instructions: &str) -> InstructionBundle {
    let trimmed = user_instructions.trim();
    let developer = if trimmed.is_empty() {
        EXECUTION_POLICY.to_string()
    } else {
        format!("{trimmed}\n\n{EXECUTION_POLICY}")
    };
    InstructionBundle {
        base: EXECUTION_POLICY.to_string(),
        developer,
    }
}

async fn cleanup_run(client: CodexAppServerClient, mcp: ComputerMcpServer) {
    let _ = tokio::time::timeout(Duration::from_secs(30), client.shutdown()).await;
    mcp.shutdown().await;
}

async fn map_boot_failure(
    shared: &SharedRunDeps,
    ctx: &AgentLoopContext,
    err: CodexProviderError,
) -> Result<(), RuntimeError> {
    let (code, message) = codex_error_to_run(err);
    fail_run(shared, ctx, &code, &message, "", 0).await?;
    Ok(())
}

fn codex_error_to_run(err: CodexProviderError) -> (String, String) {
    match err {
        CodexProviderError::Account(msg) => (msg.clone(), msg),
        CodexProviderError::CodexNotInstalled => (
            "codex_not_installed".into(),
            "Codex executable not found".into(),
        ),
        CodexProviderError::Timeout(method) => (
            "codex_timeout".into(),
            format!("Codex request timed out: {method}"),
        ),
        CodexProviderError::UnsupportedCodexToolExposure(msg) => {
            ("unsupported_codex_tool_exposure".into(), msg)
        }
        other => ("codex_provider_error".into(), other.to_string()),
    }
}

async fn consume_turn_notifications(
    client: &CodexAppServerClient,
    shared: &SharedRunDeps,
    ctx: &AgentLoopContext,
    state: Arc<Mutex<TurnRunState>>,
    notifications: &mut tokio::sync::broadcast::Receiver<RpcMessage>,
    cancel: Arc<AtomicBool>,
    interrupt_sent: Arc<AtomicBool>,
    interrupt_grace: Duration,
) -> Result<TurnOutcome, RuntimeError> {
    loop {
        let message = tokio::select! {
            msg = notifications.recv() => msg,
            _ = async {
                while !cancel.load(Ordering::Relaxed) {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            } => {
                if !interrupt_sent.swap(true, Ordering::SeqCst) {
                    let active = state.lock().await;
                    let _ = client
                        .turn_interrupt(&active.thread_id, &active.turn_id, Duration::from_secs(30))
                        .await;
                }
                continue;
            }
        };

        let message = match message {
            Ok(msg) => msg,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(_) => {
                return Ok(TurnOutcome::ProtocolError(
                    "Codex notification stream closed".into(),
                ));
            }
        };

        if let RpcMessage::Notification { method, params } = message {
            let active = state.lock().await;
            if !notification_matches_active(&active.thread_id, &active.turn_id, &params) {
                continue;
            }
            drop(active);
            match method.as_str() {
                "item/started" => {
                    if let Some(item) = item_from_notification(&params) {
                        handle_item_started(shared, ctx, &state, item).await?;
                    }
                }
                "item/completed" => {
                    if let Some(item) = item_from_notification(&params) {
                        handle_item_completed(shared, ctx, &state, item).await?;
                    }
                }
                "item/agentMessage/delta" => {
                    handle_agent_delta(&state, &params).await?;
                }
                "turn/completed" => {
                    let _ = interrupt_grace;
                    {
                        let mut active = state.lock().await;
                        active.last_turn_completed = Some(params.clone());
                    }
                    return parse_turn_outcome(&state, &params, cancel.load(Ordering::Relaxed))
                        .await;
                }
                _ => {}
            }
        }
    }
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

async fn handle_item_started(
    shared: &SharedRunDeps,
    ctx: &AgentLoopContext,
    state: &Arc<Mutex<TurnRunState>>,
    item: &Value,
) -> Result<(), RuntimeError> {
    let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if is_disallowed_host_item(item_type) {
        state.lock().await.host_tool_violation = true;
        return Ok(());
    }
    if item_type == "agentMessage" {
        state.lock().await.assistant.on_item_started(item);
        return Ok(());
    }
    if item_type != "mcpToolCall" {
        return Ok(());
    }
    let server = item.get("server").and_then(|v| v.as_str()).unwrap_or("");
    if server != MCP_SERVER_NAME {
        return Ok(());
    }
    let tool = item
        .get("tool")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let call_id = item
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let mut active = state.lock().await;
    if !active.tool_calls_seen.insert(call_id.clone()) {
        return Ok(());
    }
    active.step_count += 1;
    let step_count = active.step_count;
    shared
        .store
        .update_run(&ctx.request_id, "running", None, step_count)
        .await?;

    let arguments = item.get("arguments").cloned().unwrap_or(json!({}));
    let call_body = json!({
        "server": MCP_SERVER_NAME,
        "tool": tool,
        "callId": call_id,
        "arguments": arguments
    });
    let call_message = persist_event(
        shared,
        ctx,
        "tool_call",
        &call_body.to_string(),
        MessageStatus::Complete,
        Some("tool_call"),
        &call_body,
    )
    .await?;
    shared.events.emit(agent_core::AgentEvent::ToolCall {
        tool: tool.clone(),
        call_id: call_id.clone(),
        arguments,
        message: Some(call_message),
    })?;
    Ok(())
}

async fn handle_item_completed(
    shared: &SharedRunDeps,
    ctx: &AgentLoopContext,
    state: &Arc<Mutex<TurnRunState>>,
    item: &Value,
) -> Result<(), RuntimeError> {
    let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if is_disallowed_host_item(item_type) {
        state.lock().await.host_tool_violation = true;
        return Ok(());
    }
    if item_type == "agentMessage" {
        state
            .lock()
            .await
            .assistant
            .on_agent_message_completed(item);
        return Ok(());
    }
    if item_type != "mcpToolCall" {
        return Ok(());
    }
    let server = item.get("server").and_then(|v| v.as_str()).unwrap_or("");
    if server != MCP_SERVER_NAME {
        return Ok(());
    }
    let tool = item
        .get("tool")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let call_id = item
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let mut active = state.lock().await;
    if !active.tool_results_seen.insert(call_id.clone()) {
        return Ok(());
    }
    let status = item.get("status").and_then(|v| v.as_str()).unwrap_or("");
    let ok = status == "completed" || item.get("success").and_then(|v| v.as_bool()) == Some(true);
    let duration_ms = item.get("durationMs").cloned().unwrap_or(json!(0));
    let output = mcp_tool_output_string(item);
    let result_body = json!({
        "server": MCP_SERVER_NAME,
        "tool": tool,
        "callId": call_id,
        "ok": ok,
        "durationMs": duration_ms,
        "output": output
    });
    let result_message = persist_event(
        shared,
        ctx,
        "tool_result",
        &result_body.to_string(),
        if ok {
            MessageStatus::Complete
        } else {
            MessageStatus::Error
        },
        Some("tool_result"),
        &result_body,
    )
    .await?;
    shared.events.emit(agent_core::AgentEvent::ToolResult {
        tool,
        call_id,
        ok,
        output,
        message: Some(result_message),
    })?;
    Ok(())
}

async fn handle_agent_delta(
    state: &Arc<Mutex<TurnRunState>>,
    params: &Value,
) -> Result<(), RuntimeError> {
    state.lock().await.assistant.on_agent_message_delta(params);
    Ok(())
}

fn mcp_tool_output_string(item: &Value) -> String {
    if let Some(result) = item.get("result") {
        return serde_json::to_string(result).unwrap_or_else(|_| result.to_string());
    }
    item.get("output")
        .and_then(|v| v.as_str())
        .unwrap_or("{}")
        .to_string()
}

fn is_disallowed_host_item(item_type: &str) -> bool {
    matches!(
        item_type,
        "commandExecution" | "fileChange" | "applyPatch" | "shell" | "localShell"
    )
}

async fn parse_turn_outcome(
    state: &Arc<Mutex<TurnRunState>>,
    params: &Value,
    cancelled: bool,
) -> Result<TurnOutcome, RuntimeError> {
    let active = state.lock().await;
    if active.host_tool_violation {
        return Ok(TurnOutcome::HostToolViolation);
    }
    let (_, _, status) = parse_turn_completed(params)
        .map_err(|e| RuntimeError::Model(format!("turn/completed parse error: {e}")))?;
    match status.as_str() {
        "completed" => Ok(TurnOutcome::Completed),
        "failed" => {
            let message = turn_error_message(params).unwrap_or_else(|| "Codex turn failed".into());
            Ok(TurnOutcome::Failed {
                code: "codex_turn_failed".into(),
                message,
            })
        }
        "interrupted" => {
            if cancelled {
                Ok(TurnOutcome::Cancelled)
            } else {
                Ok(TurnOutcome::Interrupted {
                    code: "codex_turn_interrupted".into(),
                })
            }
        }
        "inProgress" => Ok(TurnOutcome::ProtocolError(
            "turn/completed reported inProgress".into(),
        )),
        other => Ok(TurnOutcome::ProtocolError(format!(
            "unknown turn status: {other}"
        ))),
    }
}
