use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::error::CodexProviderError;
use crate::process::ManagedCodexProcess;
use crate::protocol::rpc::{parse_request_id, response_envelope, RequestId};

pub async fn spawn_fake_app_server() -> Result<ManagedCodexProcess, CodexProviderError> {
    let (mut client_writer, server_reader) = tokio::io::duplex(65536);
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
            let result = handle_fake_request(&method, params);
            let payload = response_envelope(&id, result).to_string();
            let _ = writer.write_all(payload.as_bytes()).await;
            let _ = writer.write_all(b"\n").await;
            let _ = writer.flush().await;
        }
    });

    ManagedCodexProcess::from_async_io(client_writer, client_reader, stderr_writer).await
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
            "requiresOpenaiAuth": false
        }),
        "account/login/start" => serde_json::json!({
            "type": "chatgpt",
            "loginId": "login-1",
            "authUrl": "https://example.com/auth"
        }),
        "account/login/cancel" => serde_json::json!({}),
        "account/rateLimits/read" => serde_json::json!({ "planType": "pro", "allowed": true }),
        "thread/start" => serde_json::json!({ "threadId": "thread-1" }),
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
        _ => serde_json::json!({ "echoMethod": method, "echoParams": params }),
    }
}
