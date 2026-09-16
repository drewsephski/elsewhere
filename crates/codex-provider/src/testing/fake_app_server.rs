use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use agent_core::ALL_AGENT_TOOL_NAMES;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{Mutex, Notify};

use crate::error::CodexProviderError;
use crate::process::ManagedCodexProcess;
use crate::protocol::rpc::{notification_envelope, parse_request_id, response_envelope, RequestId};

pub async fn spawn_fake_app_server() -> Result<ManagedCodexProcess, CodexProviderError> {
    spawn_fake_app_server_with_mode(FakeServerMode::HappyPath).await
}

pub async fn spawn_fake_app_server_with_mode(
    mode: FakeServerMode,
) -> Result<ManagedCodexProcess, CodexProviderError> {
    let (client_writer, server_reader) = tokio::io::duplex(65536);
    let (server_writer, client_reader) = tokio::io::duplex(65536);
    let (_stderr_reader, stderr_writer) = tokio::io::duplex(1024);
    let state = Arc::new(FakeServerState::new());
    let writer = Arc::new(Mutex::new(server_writer));

    tokio::spawn(async move {
        let mut reader = BufReader::new(server_reader);
        let mut line = String::new();
        loop {
            line.clear();
            if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                break;
            }
            let Some((id, method, params)) = parse_client_request(&line) else {
                continue;
            };
            if method == "initialized" {
                continue;
            }
            let result = handle_fake_request(&method, params.clone(), &state);
            {
                let mut sink = writer.lock().await;
                let payload = response_envelope(&id, result.clone()).to_string();
                let _ = sink.write_all(payload.as_bytes()).await;
                let _ = sink.write_all(b"\n").await;
                let _ = sink.flush().await;
            }

            if method != "turn/start" {
                continue;
            }
            let thread_id = params
                .get("threadId")
                .and_then(|v| v.as_str())
                .unwrap_or("thread-1")
                .to_string();
            let turn_id = result
                .get("turn")
                .and_then(|turn| turn.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or("turn-1")
                .to_string();
            let role = state.role_for_turn(&thread_id, &turn_id);
            let emit_state = state.clone();
            let emit_writer = writer.clone();
            let turn_params = params.clone();
            tokio::spawn(async move {
                if let Err(err) = emit_turn_sequence(
                    emit_writer,
                    emit_state,
                    mode,
                    role,
                    thread_id,
                    turn_id,
                    turn_params,
                )
                .await
                {
                    tracing::debug!("fake app-server turn sequence ended: {err}");
                }
            });
        }
    });

    ManagedCodexProcess::from_async_io(client_writer, client_reader, stderr_writer).await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeServerMode {
    HappyPath,
    TextOnly,
    TurnFailed,
    TurnInterrupted,
    WrongThreadNotifications,
    NestedChildWhileParentTurnOpen,
    EchoUserInput,
    AskUserThenContinue,
}

struct FakeServerState {
    next_thread: AtomicU32,
    next_turn: AtomicU32,
    parent_recorded: AtomicU8,
    parent_thread: std::sync::Mutex<Option<String>>,
    parent_turn: std::sync::Mutex<Option<String>>,
    child_finished: AtomicU8,
    child_done: Notify,
}

impl FakeServerState {
    fn new() -> Self {
        Self {
            next_thread: AtomicU32::new(0),
            next_turn: AtomicU32::new(0),
            parent_recorded: AtomicU8::new(0),
            parent_thread: std::sync::Mutex::new(None),
            parent_turn: std::sync::Mutex::new(None),
            child_finished: AtomicU8::new(0),
            child_done: Notify::new(),
        }
    }

    fn next_thread_id(&self) -> String {
        let n = self.next_thread.fetch_add(1, Ordering::SeqCst) + 1;
        format!("thread-{n}")
    }

    fn next_turn_id(&self) -> String {
        let n = self.next_turn.fetch_add(1, Ordering::SeqCst) + 1;
        format!("turn-{n}")
    }

    fn record_turn(&self, thread_id: &str, turn_id: &str) {
        if self
            .parent_recorded
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            *self.parent_thread.lock().expect("parent thread") = Some(thread_id.to_string());
            *self.parent_turn.lock().expect("parent turn") = Some(turn_id.to_string());
        }
    }

    fn role_for_turn(&self, thread_id: &str, turn_id: &str) -> TurnRole {
        let parent_thread = self.parent_thread.lock().expect("parent thread").clone();
        let parent_turn = self.parent_turn.lock().expect("parent turn").clone();
        if parent_thread.as_deref() == Some(thread_id) && parent_turn.as_deref() == Some(turn_id) {
            TurnRole::Parent
        } else {
            TurnRole::Child
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TurnRole {
    Parent,
    Child,
}

fn parse_client_request(line: &str) -> Option<(RequestId, String, serde_json::Value)> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.get("result").is_some() || value.get("error").is_some() {
        return None;
    }
    let id = parse_request_id(value.get("id")).ok()?;
    let method = value.get("method")?.as_str()?.to_string();
    let params = value
        .get("params")
        .cloned()
        .unwrap_or(serde_json::json!({}));
    Some((id, method, params))
}

fn handle_fake_request(
    method: &str,
    params: serde_json::Value,
    state: &FakeServerState,
) -> serde_json::Value {
    match method {
        "initialize" => serde_json::json!({
            "userAgent": "fake/1.0",
            "codexHome": "/tmp/fake-codex",
            "platformFamily": "unix",
            "platformOs": "macos"
        }),
        "account/read" => serde_json::json!({
            "account": { "type": "chatgpt", "email": "user@example.com", "planType": "pro" },
            "requiresOpenaiAuth": true
        }),
        "account/login/start" => serde_json::json!({
            "type": "chatgpt",
            "loginId": "login-1",
            "authUrl": "https://example.com/auth"
        }),
        "account/login/cancel" => serde_json::json!({}),
        "account/rateLimits/read" => serde_json::json!({ "planType": "pro", "allowed": true }),
        "thread/start" => serde_json::json!({ "threadId": state.next_thread_id() }),
        "thread/resume" => {
            let id = params
                .get("threadId")
                .and_then(|v| v.as_str())
                .unwrap_or("thread-1");
            serde_json::json!({ "threadId": id })
        }
        "thread/compact/start" => serde_json::json!({}),
        "mcpServerStatus/list" => {
            let mut tools = serde_json::Map::new();
            for name in ALL_AGENT_TOOL_NAMES {
                tools.insert((*name).to_string(), serde_json::json!({}));
            }
            serde_json::json!({
                "data": [{
                    "name": "elsewhere",
                    "runtimeStatus": "connected",
                    "tools": tools
                }]
            })
        }
        "turn/start" => {
            let thread_id = params
                .get("threadId")
                .and_then(|v| v.as_str())
                .unwrap_or("thread-1");
            let turn_id = state.next_turn_id();
            state.record_turn(thread_id, &turn_id);
            serde_json::json!({
                "turn": { "id": turn_id, "status": "inProgress", "items": [] }
            })
        }
        "turn/interrupt" => serde_json::json!({}),
        _ => serde_json::json!({ "echoMethod": method, "echoParams": params }),
    }
}

async fn emit_turn_sequence(
    writer: Arc<Mutex<impl AsyncWriteExt + Unpin + Send>>,
    state: Arc<FakeServerState>,
    mode: FakeServerMode,
    role: TurnRole,
    thread_id: String,
    turn_id: String,
    params: serde_json::Value,
) -> Result<(), CodexProviderError> {
    if mode == FakeServerMode::EchoUserInput {
        let input = params
            .get("input")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let has_image = input
            .iter()
            .any(|item| item.get("type").and_then(|t| t.as_str()) == Some("localImage"));
        let summary = if has_image {
            "native localImage received"
        } else {
            "text-only user input"
        };
        return emit_text_turn(&writer, mode, &thread_id, &turn_id, summary, false).await;
    }

    if mode == FakeServerMode::AskUserThenContinue {
        write_notification(
            &writer,
            "item/started",
            serde_json::json!({
                "threadId": thread_id,
                "turnId": turn_id,
                "item": {
                    "type": "mcpToolCall",
                    "id": "item-ask",
                    "server": "elsewhere",
                    "tool": "ask_user",
                    "arguments": {
                        "question": "Which environment should I deploy to?",
                        "options": ["Staging", "Production", "Don't deploy"]
                    },
                    "status": "inProgress"
                }
            }),
        )
        .await?;
        write_notification(
            &writer,
            "item/completed",
            serde_json::json!({
                "threadId": thread_id,
                "turnId": turn_id,
                "item": {
                    "type": "mcpToolCall",
                    "id": "item-ask",
                    "server": "elsewhere",
                    "tool": "ask_user",
                    "status": "completed",
                    "success": true,
                    "durationMs": 20,
                    "result": { "selectedIndex": 0, "selectedOption": "Staging" }
                }
            }),
        )
        .await?;
        return emit_text_turn(
            &writer,
            mode,
            &thread_id,
            &turn_id,
            "Deploying to Staging",
            false,
        )
        .await;
    }
    if mode == FakeServerMode::NestedChildWhileParentTurnOpen && role == TurnRole::Parent {
        let notified = state.child_done.notified();
        if state.child_finished.load(Ordering::SeqCst) == 0 {
            match tokio::time::timeout(Duration::from_secs(8), notified).await {
                Ok(()) => {}
                Err(_) => {
                    return emit_completed(
                        &writer,
                        &thread_id,
                        &turn_id,
                        "failed",
                        Some("nested child did not complete"),
                        None,
                    )
                    .await;
                }
            }
        }
        return emit_text_turn(&writer, mode, &thread_id, &turn_id, "parent resumed", false).await;
    }

    if mode == FakeServerMode::NestedChildWhileParentTurnOpen && role == TurnRole::Child {
        emit_text_turn(
            &writer,
            mode,
            &thread_id,
            &turn_id,
            "helper findings",
            false,
        )
        .await?;
        state.child_finished.store(1, Ordering::SeqCst);
        state.child_done.notify_waiters();
        return Ok(());
    }

    emit_standard_turn(&writer, mode, &thread_id, &turn_id).await
}

async fn emit_standard_turn(
    writer: &Arc<Mutex<impl AsyncWriteExt + Unpin + Send>>,
    mode: FakeServerMode,
    thread_id: &str,
    turn_id: &str,
) -> Result<(), CodexProviderError> {
    let wrong_thread = if mode == FakeServerMode::WrongThreadNotifications {
        "other-thread"
    } else {
        thread_id
    };

    if mode != FakeServerMode::WrongThreadNotifications && mode != FakeServerMode::TextOnly {
        write_notification(
            writer,
            "item/started",
            serde_json::json!({
                "threadId": thread_id,
                "turnId": turn_id,
                "item": {
                    "type": "mcpToolCall",
                    "id": "item-write",
                    "server": "elsewhere",
                    "tool": "workspace_write",
                    "arguments": { "path": "/workspace/a.txt", "content": "hi" },
                    "status": "inProgress"
                }
            }),
        )
        .await?;
        write_notification(
            writer,
            "item/completed",
            serde_json::json!({
                "threadId": thread_id,
                "turnId": turn_id,
                "item": {
                    "type": "mcpToolCall",
                    "id": "item-write",
                    "server": "elsewhere",
                    "tool": "workspace_write",
                    "status": "completed",
                    "success": true,
                    "durationMs": 12,
                    "result": { "ok": true }
                }
            }),
        )
        .await?;
    }

    if mode != FakeServerMode::WrongThreadNotifications {
        write_notification(
            writer,
            "item/agentMessage/delta",
            serde_json::json!({
                "threadId": thread_id,
                "turnId": turn_id,
                "itemId": "msg-1",
                "delta": "hello "
            }),
        )
        .await?;
        write_notification(
            writer,
            "item/agentMessage/delta",
            serde_json::json!({
                "threadId": thread_id,
                "turnId": turn_id,
                "itemId": "msg-1",
                "delta": "world"
            }),
        )
        .await?;
        write_notification(
            writer,
            "item/completed",
            serde_json::json!({
                "threadId": thread_id,
                "turnId": turn_id,
                "item": {
                    "type": "agentMessage",
                    "id": "msg-1",
                    "text": "hello world"
                }
            }),
        )
        .await?;
    } else {
        write_notification(
            writer,
            "item/started",
            serde_json::json!({
                "threadId": wrong_thread,
                "turnId": turn_id,
                "item": {
                    "type": "mcpToolCall",
                    "id": "ignored",
                    "server": "elsewhere",
                    "tool": "workspace_write",
                    "arguments": {},
                    "status": "inProgress"
                }
            }),
        )
        .await?;
    }

    let status = match mode {
        FakeServerMode::HappyPath
        | FakeServerMode::TextOnly
        | FakeServerMode::WrongThreadNotifications
        | FakeServerMode::NestedChildWhileParentTurnOpen
        | FakeServerMode::EchoUserInput
        | FakeServerMode::AskUserThenContinue => "completed",
        FakeServerMode::TurnFailed => "failed",
        FakeServerMode::TurnInterrupted => "interrupted",
    };
    emit_completed(writer, thread_id, turn_id, status, None, None).await
}

async fn emit_text_turn(
    writer: &Arc<Mutex<impl AsyncWriteExt + Unpin + Send>>,
    _mode: FakeServerMode,
    thread_id: &str,
    turn_id: &str,
    text: &str,
    failed: bool,
) -> Result<(), CodexProviderError> {
    write_notification(
        writer,
        "item/agentMessage/delta",
        serde_json::json!({
            "threadId": thread_id,
            "turnId": turn_id,
            "itemId": "msg-1",
            "delta": text
        }),
    )
    .await?;
    write_notification(
        writer,
        "item/completed",
        serde_json::json!({
            "threadId": thread_id,
            "turnId": turn_id,
            "item": {
                "type": "agentMessage",
                "id": "msg-1",
                "text": text
            }
        }),
    )
    .await?;
    let status = if failed { "failed" } else { "completed" };
    emit_completed(writer, thread_id, turn_id, status, None, None).await
}

async fn emit_completed(
    writer: &Arc<Mutex<impl AsyncWriteExt + Unpin + Send>>,
    thread_id: &str,
    turn_id: &str,
    status: &str,
    error: Option<&str>,
    _unused: Option<()>,
) -> Result<(), CodexProviderError> {
    let mut turn = serde_json::json!({
        "id": turn_id,
        "status": status,
        "items": []
    });
    if status == "failed" {
        turn["error"] = serde_json::json!({
            "message": error.unwrap_or("simulated failure")
        });
    }
    write_notification(
        writer,
        "turn/completed",
        serde_json::json!({
            "threadId": thread_id,
            "turn": turn
        }),
    )
    .await
}

async fn write_notification(
    writer: &Arc<Mutex<impl AsyncWriteExt + Unpin + Send>>,
    method: &str,
    params: serde_json::Value,
) -> Result<(), CodexProviderError> {
    let payload = notification_envelope(method, Some(params)).to_string();
    let mut sink = writer.lock().await;
    sink.write_all(payload.as_bytes())
        .await
        .map_err(|e| CodexProviderError::Process(e.to_string()))?;
    sink.write_all(b"\n")
        .await
        .map_err(|e| CodexProviderError::Process(e.to_string()))?;
    sink.flush()
        .await
        .map_err(|e| CodexProviderError::Process(e.to_string()))?;
    Ok(())
}
