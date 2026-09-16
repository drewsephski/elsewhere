use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use agent_core::{subagent_developer_instructions, SubagentTurn};
use codex_provider::{
    build_toolless_thread_start_params, spawn_fake_app_server_with_mode, CodexAppServerClient,
    FakeServerMode, IncomingMessage, NestedCodexTurn, ToollessThreadConfig,
};
use tokio::time::timeout;

#[tokio::test]
async fn nested_child_turn_uses_distinct_ids_and_does_not_launch_another_process() {
    let process = spawn_fake_app_server_with_mode(FakeServerMode::NestedChildWhileParentTurnOpen)
        .await
        .expect("fake server");
    let client = Arc::new(
        CodexAppServerClient::from_process(process)
            .await
            .expect("client"),
    );

    let cwd = PathBuf::from("/tmp");
    let parent_config = ToollessThreadConfig {
        cwd: cwd.clone(),
        model: "gpt-5.6-luna".into(),
        developer_instructions: Some("parent".into()),
    };
    let parent_thread = client
        .thread_start_toolless(&parent_config)
        .await
        .expect("parent thread");
    let mut parent_notifications = client.notifications();
    let parent_turn = client
        .turn_start(&parent_thread, "parent task", Duration::from_secs(5))
        .await
        .expect("parent turn");

    let cancel = Arc::new(AtomicBool::new(false));
    let nested = NestedCodexTurn::new(client.clone(), cwd, "gpt-5.6-luna", cancel);
    let child_cancel = AtomicBool::new(false);
    let child_text = timeout(
        Duration::from_secs(5),
        nested.run_toolless(
            "gpt-5.6-luna",
            &subagent_developer_instructions(),
            "summarize",
            &child_cancel,
        ),
    )
    .await
    .expect("child timed out")
    .expect("child result");
    assert!(
        child_text.contains("helper findings"),
        "child text was {child_text}"
    );
    assert!(
        !child_text.contains("parent resumed"),
        "child accumulator saw parent text: {child_text}"
    );

    let mut saw_parent_resume = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let Ok(Ok(message)) = timeout(remaining, parent_notifications.recv()).await else {
            break;
        };
        let IncomingMessage::Notification { method, params } = message else {
            continue;
        };
        if method != "turn/completed" {
            continue;
        }
        let thread = params.get("threadId").and_then(|v| v.as_str());
        let turn = params
            .get("turn")
            .and_then(|t| t.get("id"))
            .and_then(|v| v.as_str());
        if thread == Some(parent_thread.as_str()) && turn == Some(parent_turn.as_str()) {
            saw_parent_resume = true;
            break;
        }
    }
    assert!(saw_parent_resume, "parent turn should complete after child");
    assert!(parent_thread.starts_with("thread-"));
    assert!(parent_turn.starts_with("turn-"));

    client.shutdown().await.expect("shutdown");
}

#[test]
fn helper_thread_config_has_no_mcp_or_computer_tools() {
    let params = build_toolless_thread_start_params(&ToollessThreadConfig {
        cwd: PathBuf::from("/tmp/elsewhere-toolless"),
        model: "gpt-5.6-luna".into(),
        developer_instructions: Some(subagent_developer_instructions()),
    })
    .expect("params");
    let encoded = params.to_string();
    assert!(!encoded.contains("run_subagent"));
    assert!(!encoded.contains("mcp_servers"));
    assert!(!encoded.contains("workspace_exec"));
}
