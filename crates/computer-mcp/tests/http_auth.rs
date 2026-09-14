use std::sync::Arc;

use agent_core::{AgentComputer, FakeAgentComputer};
use computer_mcp::ComputerMcpServer;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};

#[tokio::test]
async fn mcp_tools_require_bearer_token() {
    let server = ComputerMcpServer::start(Arc::new(FakeAgentComputer::new()))
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
    assert!(authorized.status().is_success(), "status {}", authorized.status());

    server.shutdown().await;
}

#[tokio::test]
async fn fake_computer_rejects_outside_workspace_via_mcp_session() {
    let computer = Arc::new(FakeAgentComputer::new());
    let _ = computer
        .write_file("/workspace/a.txt", b"hi")
        .await
        .expect("seed");
    let server = ComputerMcpServer::start(computer)
        .await
        .expect("start");
    assert!(server.url().starts_with("http://127.0.0.1:"));
    server.shutdown().await;
}
