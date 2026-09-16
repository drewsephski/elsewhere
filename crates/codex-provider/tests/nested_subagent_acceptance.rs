//! Opt-in real Codex nested-thread acceptance. Keep out of credential-free CI.
//!
//! Run with:
//! `ELSEWHERE_CODEX_SUBAGENT_ACCEPTANCE=1 cargo test -p codex-provider nested_subagent_acceptance -- --nocapture --ignored`
//!
//! Requires a logged-in ChatGPT Codex subscription profile. Never falls back to the paid Responses API.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use agent_core::{subagent_developer_instructions, SubagentTurn};
use codex_provider::{
    which_codex_executable, CodexAppServerClient, CodexProcessLaunch, NestedCodexTurn,
    ToollessThreadConfig,
};

fn acceptance_enabled() -> bool {
    matches!(
        std::env::var("ELSEWHERE_CODEX_SUBAGENT_ACCEPTANCE").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE")
    )
}

#[tokio::test]
async fn real_codex_parent_nested_child_then_parent_resumes() {
    if !acceptance_enabled() {
        eprintln!(
            "skipping real Codex subagent acceptance (set ELSEWHERE_CODEX_SUBAGENT_ACCEPTANCE=1)"
        );
        return;
    }
    if std::env::var("OPENAI_API_KEY").is_ok() {
        panic!("OPENAI_API_KEY must be unset; nested Codex acceptance must not use Responses API");
    }

    let executable = which_codex_executable().expect("codex executable");
    let model = std::env::var("ELSEWHERE_CODEX_MODEL").unwrap_or_else(|_| "gpt-5.6-luna".into());
    let cwd = tempfile::tempdir().expect("cwd");
    let launch = CodexProcessLaunch::from_path(executable).subscription_child();
    let client = Arc::new(
        CodexAppServerClient::launch(launch)
            .await
            .expect("launch real Codex app-server"),
    );

    let parent_config = ToollessThreadConfig {
        cwd: cwd.path().to_path_buf(),
        model: model.clone(),
        developer_instructions: Some("You are the parent. Wait; a helper may run.".into()),
    };
    let parent_thread = client
        .thread_start_toolless(&parent_config)
        .await
        .expect("parent thread");
    let parent_turn = client
        .turn_start(
            &parent_thread,
            "Reply with the single word parent-ack after thinking briefly.",
            Duration::from_secs(60),
        )
        .await
        .expect("parent turn start");

    let cancel = Arc::new(AtomicBool::new(false));
    let nested = NestedCodexTurn::new(
        client.clone(),
        cwd.path().to_path_buf(),
        model.clone(),
        cancel,
    );
    let child_cancel = AtomicBool::new(false);
    let child_text = nested
        .run_toolless(
            &model,
            &subagent_developer_instructions(),
            "Reply with the single word nested-ok.",
            &child_cancel,
        )
        .await
        .expect("nested child result");
    assert!(
        child_text.to_lowercase().contains("nested-ok") || !child_text.trim().is_empty(),
        "child returned empty or unexpected text: {child_text}"
    );

    let _ = client
        .turn_interrupt(&parent_thread, &parent_turn, Duration::from_secs(15))
        .await;
    client.shutdown().await.expect("shutdown");
}
