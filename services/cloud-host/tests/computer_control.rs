use agent_core::{
    dispatch_tool_with_gate, AgentComputer, ComputerError, ComputerInfo, ExecResult,
    ToolRunContext, WorkspaceEntry,
};
use async_trait::async_trait;
use cloud_host::approval::BrowserHumanControlGate;
use cloud_host::computer_control::{self, ControlHolder};
use cloud_host::db::resources::insert_computer_placeholder;
use sqlx::PgPool;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

async fn try_test_pool() -> Option<PgPool> {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://elsewhere:elsewhere@127.0.0.1:5432/elsewhere".into());
    let pool = tokio::time::timeout(Duration::from_secs(2), PgPool::connect(&url))
        .await
        .ok()?
        .ok()?;
    sqlx::migrate!("./migrations").run(&pool).await.ok()?;
    Some(pool)
}

struct RecordingBrowserComputer {
    calls: AtomicUsize,
}

#[async_trait]
impl AgentComputer for RecordingBrowserComputer {
    async fn ensure_ready(&self) -> Result<ComputerInfo, ComputerError> {
        Ok(ComputerInfo {
            ready: true,
            protocol_version: 1,
            detail: None,
        })
    }

    async fn list_dir(
        &self,
        _path: &str,
    ) -> Result<Vec<WorkspaceEntry>, ComputerError> {
        Ok(vec![])
    }

    async fn read_file(&self, _path: &str) -> Result<Vec<u8>, ComputerError> {
        Ok(vec![])
    }

    async fn write_file(&self, _path: &str, _data: &[u8]) -> Result<(), ComputerError> {
        Ok(())
    }

    async fn exec(&self, _command: &str) -> Result<ExecResult, ComputerError> {
        Ok(ExecResult {
            ok: true,
            stdout: String::new(),
            stderr: String::new(),
            exit_code: 0,
        })
    }

    async fn browser_invoke(
        &self,
        action: &str,
        _args: &serde_json::Value,
    ) -> Result<serde_json::Value, ComputerError> {
        if action == "click" {
            self.calls.fetch_add(1, Ordering::SeqCst);
        }
        Ok(serde_json::json!({ "ok": true, "action": action }))
    }
}

fn test_run(owner: &str, computer_id: &str) -> ToolRunContext {
    ToolRunContext {
        run_id: "run-1".into(),
        request_id: "req-1".into(),
        owner_id: owner.into(),
        bot_id: "bot-1".into(),
        computer_id: computer_id.into(),
        tool_invocation_id: None,
    }
}

#[tokio::test]
async fn agent_browser_mutation_waits_until_human_returns_control() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping agent_browser_mutation_waits_until_human_returns_control");
        return;
    };
    let owner = format!("owner-{}", Uuid::new_v4());
    let computer = insert_computer_placeholder(&pool, &owner, "gate")
        .await
        .unwrap();
    computer_control::take_human_control(&pool, &owner, &computer.id)
        .await
        .unwrap();

    let cancel = Arc::new(AtomicBool::new(false));
    let gate = BrowserHumanControlGate::wrapping_allow_all(pool.clone(), cancel.clone());
    let recording = RecordingBrowserComputer {
        calls: AtomicUsize::new(0),
    };

    let owner_for_release = owner.clone();
    let pool_for_release = pool.clone();
    let computer_id = computer.id.clone();
    let release = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(600)).await;
        computer_control::return_control_to_bot(&pool_for_release, &owner_for_release, &computer_id)
            .await
            .unwrap();
    });

    let result = dispatch_tool_with_gate(
        &recording,
        "browser_click",
        r#"{"ref":"e1"}"#,
        &cancel,
        &gate,
        &test_run(&owner, &computer.id),
    )
    .await
    .expect("click after human release");

    release.await.unwrap();
    assert_eq!(result.get("ok"), Some(&serde_json::json!(true)));
    assert_eq!(recording.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn human_browser_mutation_rejected_without_control() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping human_browser_mutation_rejected_without_control");
        return;
    };
    let owner = format!("owner-{}", Uuid::new_v4());
    let computer = insert_computer_placeholder(&pool, &owner, "human-api")
        .await
        .unwrap();
    let err = computer_control::require_active_human_control(&pool, &owner, &computer.id)
        .await
        .unwrap_err();
    assert!(matches!(err, computer_control::HumanControlRequired::NotHuman));
}

#[tokio::test]
async fn returning_control_restores_bot_holder() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping returning_control_restores_bot_holder");
        return;
    };
    let owner = format!("owner-{}", Uuid::new_v4());
    let computer = insert_computer_placeholder(&pool, &owner, "return")
        .await
        .unwrap();
    computer_control::take_human_control(&pool, &owner, &computer.id)
        .await
        .unwrap();
    let state = computer_control::return_control_to_bot(&pool, &owner, &computer.id)
        .await
        .unwrap();
    assert_eq!(state.holder, ControlHolder::Bot);
}

#[tokio::test]
async fn stale_human_lease_recovers_to_bot() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping stale_human_lease_recovers_to_bot");
        return;
    };
    let owner = format!("owner-{}", Uuid::new_v4());
    let computer = insert_computer_placeholder(&pool, &owner, "stale")
        .await
        .unwrap();
    computer_control::take_human_control(&pool, &owner, &computer.id)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE computer_control_leases SET heartbeat_at = NOW() - INTERVAL '10 minutes' WHERE computer_id = $1",
    )
    .bind(&computer.id)
    .execute(&pool)
    .await
    .unwrap();
    let state = computer_control::get_control_state(&pool, &owner, &computer.id)
        .await
        .unwrap();
    assert_eq!(state.holder, ControlHolder::Bot);
}

#[tokio::test]
async fn owner_isolation_on_take_control() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping owner_isolation_on_take_control");
        return;
    };
    let owner_a = format!("owner-a-{}", Uuid::new_v4());
    let owner_b = format!("owner-b-{}", Uuid::new_v4());
    let computer = insert_computer_placeholder(&pool, &owner_a, "iso")
        .await
        .unwrap();
    computer_control::take_human_control(&pool, &owner_a, &computer.id)
        .await
        .unwrap();
    let err = computer_control::take_human_control(&pool, &owner_b, &computer.id)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        computer_control::TakeControlError::Conflict(_)
    ));
}
