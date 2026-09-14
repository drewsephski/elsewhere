use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::error::CodexProviderError;
use crate::process::ManagedCodexProcess;
use crate::protocol::rpc::{
    notification_envelope, parse_request_id, response_envelope, RequestId,
};

pub async fn spawn_fake_app_server() -> Result<ManagedCodexProcess, CodexProviderError> {
    spawn_fake_app_server_with_mode(FakeServerMode::HappyPath).await
}

pub async fn spawn_fake_app_server_with_mode(
    mode: FakeServerMode,
) -> Result<ManagedCodexProcess, CodexProviderError> {
    let (client_writer, server_reader) = tokio::io::duplex(65536);
    let (server_writer, client_reader) = tokio::io::duplex(65536);
    let (_stderr_reader, stderr_writer) = tokio::io::duplex(1024);

    tokio::spawn(async move {
        let mut reader = BufReader::new(server_reader);
        let mut writer = server_writer;
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
            let result = handle_fake_request(&method, params.clone());
            let payload = response_envelope(&id, result).to_string();
            let _ = writer.write_all(payload.as_bytes()).await;
            let _ = writer.write_all(b"\n").await;
            let _ = writer.flush().await;

            if method == "turn/start" {
                if let Err(err) = emit_turn_sequence(&mut writer, &params, mode).await {
                    tracing::debug!("fake app-server turn sequence ended: {err}");
                }
            }
        }
    });

    ManagedCodexProcess::from_async_io(client_writer, client_reader, stderr_writer).await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeServerMode {
    HappyPath,
    TurnFailed,
    TurnInterrupted,
    WrongThreadNotifications,
}

fn parse_client_request(line: &str) -> Option<(RequestId, String, serde_json::Value)> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.get("result").is_some() || value.get("error").is_some() {
        return None;
    }
    let id = parse_request_id(value.get("id")).ok()?;
    let method = value.get("method")?.as_str()?.to_string();
    let params = value.get("params").cloned().unwrap_or(serde_json::json!({}));
    Some((id, method, params))
}

fn handle_fake_request(method: &str, params: serde_json::Value) -> serde_json::Value {
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
        "thread/start" => serde_json::json!({ "threadId": "thread-1" }),
        "thread/resume" => {
            let id = params
                .get("threadId")
                .and_then(|v| v.as_str())
                .unwrap_or("thread-1");
            serde_json::json!({ "threadId": id })
        }
        "thread/compact/start" => serde_json::json!({}),
        "mcpServerStatus/list" => serde_json::json!({
            "data": [{
                "name": "elsewhere",
                "runtimeStatus": "connected",
                "tools": {
                    "workspace_list": {},
                    "workspace_read": {},
                    "workspace_write": {},
                    "workspace_exec": {}
                }
            }]
        }),
        "turn/start" => serde_json::json!({
            "turn": { "id": "turn-1", "status": "inProgress", "items": [] }
        }),
        "turn/interrupt" => serde_json::json!({}),
        _ => serde_json::json!({ "echoMethod": method, "echoParams": params }),
    }
}

async fn emit_turn_sequence(
    writer: &mut (impl AsyncWriteExt + Unpin),
    params: &serde_json::Value,
    mode: FakeServerMode,
) -> Result<(), CodexProviderError> {
    let thread_id = params
        .get("threadId")
        .and_then(|v| v.as_str())
        .unwrap_or("thread-1");
    let turn_id = "turn-1";

    let wrong_thread = if mode == FakeServerMode::WrongThreadNotifications {
        "other-thread"
    } else {
        thread_id
    };

    if mode != FakeServerMode::WrongThreadNotifications {
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
        FakeServerMode::HappyPath | FakeServerMode::WrongThreadNotifications => "completed",
        FakeServerMode::TurnFailed => "failed",
        FakeServerMode::TurnInterrupted => "interrupted",
    };

    let mut turn = serde_json::json!({
        "id": turn_id,
        "status": status,
        "items": []
    });
    if status == "failed" {
        turn["error"] = serde_json::json!({ "message": "simulated failure" });
    }

    write_notification(
        writer,
        "turn/completed",
        serde_json::json!({
            "threadId": thread_id,
            "turn": turn
        }),
    )
    .await?;

    Ok(())
}

async fn write_notification(
    writer: &mut (impl AsyncWriteExt + Unpin),
    method: &str,
    params: serde_json::Value,
) -> Result<(), CodexProviderError> {
    let payload = notification_envelope(method, Some(params)).to_string();
    writer
        .write_all(payload.as_bytes())
        .await
        .map_err(|e| CodexProviderError::Process(e.to_string()))?;
    writer
        .write_all(b"\n")
        .await
        .map_err(|e| CodexProviderError::Process(e.to_string()))?;
    writer
        .flush()
        .await
        .map_err(|e| CodexProviderError::Process(e.to_string()))?;
    Ok(())
}
