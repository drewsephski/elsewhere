use std::sync::Arc;

use std::sync::atomic::AtomicBool;

use agent_core::{AgentComputer, AllowAllApprovalGate, FakeAgentComputer, ToolRunContext};
use computer_mcp::ComputerMcpServer;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};

fn test_run() -> ToolRunContext {
    ToolRunContext {
        run_id: "run-test".into(),
        request_id: "req-test".into(),
        owner_id: "local".into(),
        bot_id: "bot".into(),
        computer_id: "comp".into(),
        tool_invocation_id: None,
    }
}

#[tokio::test]
async fn mcp_tools_require_bearer_token() {
    let server = ComputerMcpServer::start(
        Arc::new(FakeAgentComputer::new()),
        Arc::new(AllowAllApprovalGate),
        test_run(),
        Arc::new(AtomicBool::new(false)),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        "conv-test".into(),
    )
    .await
    .expect("start");

    let client = reqwest::Client::new();
    let init_body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"0.1.0"}}}"#;

    let unauthorized = client
        .post(server.url())
        .header(CONTENT_TYPE, "application/json")
        .body(init_body)
        .send()
        .await
        .expect("request");
    assert_eq!(unauthorized.status(), reqwest::StatusCode::UNAUTHORIZED);

    let authorized = client
        .post(server.url())
        .header(CONTENT_TYPE, "application/json")
        .header("Accept", "application/json, text/event-stream")
        .header(AUTHORIZATION, format!("Bearer {}", server.bearer_token()))
        .body(init_body)
        .send()
        .await
        .expect("request");
    assert!(
        authorized.status().is_success(),
        "status {}",
        authorized.status()
    );

    server.shutdown().await;
}

#[tokio::test]
async fn fake_computer_rejects_outside_workspace_via_mcp_session() {
    let computer = Arc::new(FakeAgentComputer::new());
    let _ = computer
        .write_file("/workspace/a.txt", b"hi")
        .await
        .expect("seed");
    let server = ComputerMcpServer::start(
        computer,
        Arc::new(AllowAllApprovalGate),
        test_run(),
        Arc::new(AtomicBool::new(false)),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        "conv-test".into(),
    )
    .await
    .expect("start");
    assert!(server.url().starts_with("http://127.0.0.1:"));
    server.shutdown().await;
}

#[tokio::test]
async fn dropping_run_revokes_its_computer_endpoint() {
    let server = ComputerMcpServer::start(
        Arc::new(FakeAgentComputer::new()),
        Arc::new(AllowAllApprovalGate),
        test_run(),
        Arc::new(AtomicBool::new(false)),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        "conv-test".into(),
    )
    .await
    .unwrap();
    let client = reqwest::Client::new();
    let url = server.url().to_string();
    assert!(client.post(&url).send().await.is_ok());
    drop(server);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if client.post(&url).send().await.is_err() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("aborted run must close its listener");
}
