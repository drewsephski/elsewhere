//! Human intervention request lifecycle, owner isolation, and handback.

use agent_core::{
    dispatch_agent_tool_with_gate, dispatch_agent_tool_with_gate_and_recovery, AgentComputer,
    AllowAllApprovalGate, BrowserRecoverySession, ComputerError, ComputerInfo, EventSink,
    ExecResult, HumanInterventionContext, HumanInterventionError, RunEventReceipt, RunStore,
    ToolRunContext, WorkspaceEntry,
};
use async_trait::async_trait;
use chrono::Utc;
use cloud_host::db::resources::{insert_bot, insert_computer_placeholder};
use cloud_host::human_intervention::{HumanInterventionService, RunScopedHumanIntervention};
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

struct RecordingStore {
    events: std::sync::Mutex<Vec<(String, String)>>,
}

#[async_trait]
impl RunStore for RecordingStore {
    async fn append_run_event(
        &self,
        request_id: &str,
        event_type: &str,
        _payload: &serde_json::Value,
    ) -> Result<RunEventReceipt, agent_core::RuntimeError> {
        self.events
            .lock()
            .unwrap()
            .push((request_id.to_string(), event_type.to_string()));
        Ok(RunEventReceipt { id: 1 })
    }

    async fn create_run(
        &self,
        _params: agent_core::CreateRunParams,
    ) -> Result<String, agent_core::RuntimeError> {
        Ok(Uuid::new_v4().to_string())
    }

    async fn persist_structured_message(
        &self,
        _input: agent_core::StructuredMessageInput,
    ) -> Result<agent_core::PersistedMessage, agent_core::RuntimeError> {
        Err(agent_core::RuntimeError::Store("not used".into()))
    }

    async fn update_assistant_message(
        &self,
        _message_id: &str,
        _body: &str,
        _status: agent_core::MessageStatus,
        _error_message: Option<&str>,
    ) -> Result<(), agent_core::RuntimeError> {
        Ok(())
    }

    async fn update_run(
        &self,
        _request_id: &str,
        _status: &str,
        _error_code: Option<&str>,
        _step_count: i64,
    ) -> Result<(), agent_core::RuntimeError> {
        Ok(())
    }

    async fn touch_conversation_and_bot(
        &self,
        _conversation_id: &str,
        _bot_id: &str,
    ) -> Result<(), agent_core::RuntimeError> {
        Ok(())
    }

    async fn get_assistant_message_body(
        &self,
        _message_id: &str,
    ) -> Result<String, agent_core::RuntimeError> {
        Ok(String::new())
    }
}

struct NoopEvents;

impl EventSink for NoopEvents {
    fn emit(&self, _event: agent_core::AgentEvent) -> Result<(), agent_core::RuntimeError> {
        Ok(())
    }

    fn emit_durable(
        &self,
        _event_id: i64,
        _event_type: &str,
        _payload: &serde_json::Value,
    ) -> Result<(), agent_core::RuntimeError> {
        Ok(())
    }
}

struct StubComputer;

#[async_trait]
impl AgentComputer for StubComputer {
    async fn ensure_ready(&self) -> Result<ComputerInfo, ComputerError> {
        Ok(ComputerInfo {
            ready: true,
            protocol_version: 1,
            detail: None,
        })
    }

    async fn list_dir(&self, _path: &str) -> Result<Vec<WorkspaceEntry>, ComputerError> {
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
}

async fn seed_run(pool: &PgPool, owner: &str, computer_id: &str) -> (String, String) {
    let bot = insert_bot(
        pool,
        owner,
        "helper",
        "help",
        "gpt-5.6-luna",
        Some(computer_id),
        "responses",
        "sky-wisp",
    )
    .await
    .unwrap();
    let conv_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO conversations (id, owner_id, bot_id, created_at, updated_at) VALUES ($1, $2, $3, NOW(), NOW())",
    )
    .bind(&conv_id)
    .bind(owner)
    .bind(&bot.id)
    .execute(pool)
    .await
    .unwrap();
    let run_id = format!("run-{}", Uuid::new_v4());
    let request_id = format!("req-{}", Uuid::new_v4());
    sqlx::query(
        r#"
        INSERT INTO agent_runs (
            id, owner_id, request_id, bot_id, conversation_id, computer_id, model, status, step_count, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, 'gpt-5.6-luna', 'running', 0, NOW(), NOW())
        "#,
    )
    .bind(&run_id)
    .bind(owner)
    .bind(&request_id)
    .bind(&bot.id)
    .bind(&conv_id)
    .bind(computer_id)
    .execute(pool)
    .await
    .unwrap();
    (run_id, request_id)
}

fn test_run(owner: &str, computer_id: &str, run_id: &str, request_id: &str) -> ToolRunContext {
    ToolRunContext {
        run_id: run_id.into(),
        request_id: request_id.into(),
        owner_id: owner.into(),
        bot_id: "bot-1".into(),
        computer_id: computer_id.into(),
        tool_invocation_id: None,
    }
}

#[tokio::test]
async fn intervention_request_persists_and_emits_event() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping intervention_request_persists_and_emits_event");
        return;
    };
    let owner = format!("owner-{}", Uuid::new_v4());
    let computer = insert_computer_placeholder(&pool, &owner, "intervention")
        .await
        .unwrap();
    let (run_id, request_id) = seed_run(&pool, &owner, &computer.id).await;
    let service = HumanInterventionService {
        pool: pool.clone(),
        registry: Arc::new(cloud_host::approval::ApprovalWaitRegistry::default()),
    };
    let store = Arc::new(RecordingStore {
        events: std::sync::Mutex::new(vec![]),
    });
    let events = Arc::new(NoopEvents);
    let cancel = Arc::new(AtomicBool::new(false));
    let backend: Arc<dyn agent_core::AgentHumanIntervention> =
        RunScopedHumanIntervention::new(service.clone(), store.clone(), events, cancel.clone());

    let owner_for_resolve = owner.clone();
    let pool_for_resolve = pool.clone();
    let computer_id = computer.id.clone();
    let resolve = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(400)).await;
        cloud_host::computer_control::take_human_control(
            &pool_for_resolve,
            &owner_for_resolve,
            &computer_id,
        )
        .await
        .unwrap();
        service
            .resolve_pending_for_computer_handback(&owner_for_resolve, &computer_id)
            .await
            .unwrap();
    });

    let outcome = backend
        .request_and_wait(
            &HumanInterventionContext {
                owner_id: owner.clone(),
                run_id: run_id.clone(),
                request_id: request_id.clone(),
                computer_id: computer.id.clone(),
            },
            "login",
            "Please sign in to continue",
            &AtomicBool::new(false),
        )
        .await
        .expect("resolved");

    resolve.await.unwrap();
    assert!(!outcome.intervention_id.is_empty());

    let row: (String, String) =
        sqlx::query_as("SELECT status, message FROM human_intervention_requests WHERE id = $1")
            .bind(&outcome.intervention_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row.0, "resolved");
    assert!(!row.1.contains("password"));

    let emitted = store.events.lock().unwrap();
    assert!(emitted
        .iter()
        .any(|(_, t)| t == "human_intervention_requested"));
}

#[tokio::test]
async fn take_control_does_not_resolve_pending_intervention() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping take_control_does_not_resolve_pending_intervention");
        return;
    };
    let owner = format!("owner-{}", Uuid::new_v4());
    let computer = insert_computer_placeholder(&pool, &owner, "take-only")
        .await
        .unwrap();
    let (run_id, _request_id) = seed_run(&pool, &owner, &computer.id).await;
    let intervention_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO human_intervention_requests (
            id, run_id, owner_id, computer_id, reason, message, status, requested_at, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, 'captcha', 'Solve CAPTCHA', 'pending', NOW(), NOW(), NOW())
        "#,
    )
    .bind(&intervention_id)
    .bind(&run_id)
    .bind(&owner)
    .bind(&computer.id)
    .execute(&pool)
    .await
    .unwrap();

    cloud_host::computer_control::take_human_control(&pool, &owner, &computer.id)
        .await
        .unwrap();

    let status: String =
        sqlx::query_scalar("SELECT status FROM human_intervention_requests WHERE id = $1")
            .bind(&intervention_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "pending");
}

#[tokio::test]
async fn owner_isolation_on_pending_lookup() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping owner_isolation_on_pending_lookup");
        return;
    };
    let owner_a = format!("owner-a-{}", Uuid::new_v4());
    let owner_b = format!("owner-b-{}", Uuid::new_v4());
    let computer = insert_computer_placeholder(&pool, &owner_a, "iso-int")
        .await
        .unwrap();
    let (run_id, _) = seed_run(&pool, &owner_a, &computer.id).await;
    let service = HumanInterventionService {
        pool: pool.clone(),
        registry: Arc::new(cloud_host::approval::ApprovalWaitRegistry::default()),
    };
    sqlx::query(
        r#"
        INSERT INTO human_intervention_requests (
            id, run_id, owner_id, computer_id, reason, message, status, requested_at, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, 'login', 'Sign in', 'pending', NOW(), NOW(), NOW())
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&run_id)
    .bind(&owner_a)
    .bind(&computer.id)
    .execute(&pool)
    .await
    .unwrap();

    let row = service
        .get_pending_for_owner_run(&owner_b, &run_id)
        .await
        .unwrap();
    assert!(row.is_none());
}

#[tokio::test]
async fn duplicate_pending_request_rejected() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping duplicate_pending_request_rejected");
        return;
    };
    let owner = format!("owner-{}", Uuid::new_v4());
    let computer = insert_computer_placeholder(&pool, &owner, "dup")
        .await
        .unwrap();
    let (run_id, _) = seed_run(&pool, &owner, &computer.id).await;
    sqlx::query(
        r#"
        INSERT INTO human_intervention_requests (
            id, run_id, owner_id, computer_id, reason, message, status, requested_at, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, 'login', 'First', 'pending', NOW(), NOW(), NOW())
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&run_id)
    .bind(&owner)
    .bind(&computer.id)
    .execute(&pool)
    .await
    .unwrap();

    let err = sqlx::query(
        r#"
        INSERT INTO human_intervention_requests (
            id, run_id, owner_id, computer_id, reason, message, status, requested_at, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, 'login', 'Second', 'pending', NOW(), NOW(), NOW())
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&run_id)
    .bind(&owner)
    .bind(&computer.id)
    .execute(&pool)
    .await
    .unwrap_err();
    assert_eq!(
        err.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("23505")
    );
}

#[tokio::test]
async fn cancellation_wakes_waiting_tool() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping cancellation_wakes_waiting_tool");
        return;
    };
    let owner = format!("owner-{}", Uuid::new_v4());
    let computer = insert_computer_placeholder(&pool, &owner, "cancel-int")
        .await
        .unwrap();
    let (run_id, request_id) = seed_run(&pool, &owner, &computer.id).await;
    let service = HumanInterventionService {
        pool: pool.clone(),
        registry: Arc::new(cloud_host::approval::ApprovalWaitRegistry::default()),
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let backend: Arc<dyn agent_core::AgentHumanIntervention> = RunScopedHumanIntervention::new(
        service.clone(),
        Arc::new(RecordingStore {
            events: std::sync::Mutex::new(vec![]),
        }),
        Arc::new(NoopEvents),
        cancel.clone(),
    );

    let cancel_flag = cancel.clone();
    let abort = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        cancel_flag.store(true, Ordering::SeqCst);
    });

    let err = backend
        .request_and_wait(
            &HumanInterventionContext {
                owner_id: owner.clone(),
                run_id: run_id.clone(),
                request_id,
                computer_id: computer.id.clone(),
            },
            "two_factor",
            "Approve the push notification",
            &AtomicBool::new(false),
        )
        .await
        .unwrap_err();
    abort.await.unwrap();
    assert!(matches!(err, HumanInterventionError::Cancelled));
}

struct CountingComputer {
    snapshots: AtomicUsize,
}

#[async_trait]
impl AgentComputer for CountingComputer {
    async fn ensure_ready(&self) -> Result<ComputerInfo, ComputerError> {
        Ok(ComputerInfo {
            ready: true,
            protocol_version: 1,
            detail: None,
        })
    }

    async fn list_dir(&self, _path: &str) -> Result<Vec<WorkspaceEntry>, ComputerError> {
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
        if action == "snapshot" {
            self.snapshots.fetch_add(1, Ordering::SeqCst);
        }
        Ok(serde_json::json!({ "ok": true }))
    }
}

#[tokio::test]
async fn tool_dispatch_blocks_until_handback() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping tool_dispatch_blocks_until_handback");
        return;
    };
    let owner = format!("owner-{}", Uuid::new_v4());
    let computer = insert_computer_placeholder(&pool, &owner, "dispatch")
        .await
        .unwrap();
    let (run_id, request_id) = seed_run(&pool, &owner, &computer.id).await;
    let service = HumanInterventionService {
        pool: pool.clone(),
        registry: Arc::new(cloud_host::approval::ApprovalWaitRegistry::default()),
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let backend: Arc<dyn agent_core::AgentHumanIntervention> = RunScopedHumanIntervention::new(
        service.clone(),
        Arc::new(RecordingStore {
            events: std::sync::Mutex::new(vec![]),
        }),
        Arc::new(NoopEvents),
        cancel.clone(),
    );
    let gate = AllowAllApprovalGate;
    let counting = CountingComputer {
        snapshots: AtomicUsize::new(0),
    };

    let owner_for_handback = owner.clone();
    let pool_for_handback = pool.clone();
    let computer_id = computer.id.clone();
    let handback = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        service
            .resolve_pending_for_computer_handback(&owner_for_handback, &computer_id)
            .await
            .unwrap();
    });

    let tool_run = test_run(&owner, &computer.id, &run_id, &request_id);
    let result = dispatch_agent_tool_with_gate(
        &counting,
        None,
        None,
        Some(&backend),
        "browser_request_human",
        r#"{"reason":"passkey","message":"Complete passkey sign-in"}"#,
        &cancel,
        &gate,
        &tool_run,
        None,
    )
    .await
    .expect("handback resolves tool");

    handback.await.unwrap();
    assert_eq!(result.get("ok"), Some(&serde_json::json!(true)));

    dispatch_agent_tool_with_gate(
        &counting,
        None,
        None,
        Some(&backend),
        "browser_snapshot",
        r#"{}"#,
        &cancel,
        &gate,
        &tool_run,
        None,
    )
    .await
    .expect("snapshot after handback");
    assert_eq!(counting.snapshots.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn recovery_session_blocks_mutation_until_snapshot_after_handback() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping recovery_session_blocks_mutation_until_snapshot_after_handback");
        return;
    };
    let owner = format!("owner-{}", Uuid::new_v4());
    let computer = insert_computer_placeholder(&pool, &owner, "recovery-gate")
        .await
        .unwrap();
    let (run_id, request_id) = seed_run(&pool, &owner, &computer.id).await;
    let service = HumanInterventionService {
        pool: pool.clone(),
        registry: Arc::new(cloud_host::approval::ApprovalWaitRegistry::default()),
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let backend: Arc<dyn agent_core::AgentHumanIntervention> = RunScopedHumanIntervention::new(
        service.clone(),
        Arc::new(RecordingStore {
            events: std::sync::Mutex::new(vec![]),
        }),
        Arc::new(NoopEvents),
        cancel.clone(),
    );
    let gate = AllowAllApprovalGate;
    let counting = CountingComputer {
        snapshots: AtomicUsize::new(0),
    };
    let recovery = Arc::new(BrowserRecoverySession::new());
    let tool_run = test_run(&owner, &computer.id, &run_id, &request_id);

    let owner_for_handback = owner.clone();
    let computer_id = computer.id.clone();
    let handback = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        service
            .resolve_pending_for_computer_handback(&owner_for_handback, &computer_id)
            .await
            .unwrap();
    });

    dispatch_agent_tool_with_gate_and_recovery(
        &counting,
        None,
        None,
        Some(&backend),
        None,
        None,
        "browser_request_human",
        r#"{"reason":"captcha","message":"Solve the CAPTCHA in the browser"}"#,
        &cancel,
        &gate,
        &tool_run,
        None,
        Some(&recovery),
    )
    .await
    .expect("human tool resolves");
    handback.await.unwrap();

    let blocked = dispatch_agent_tool_with_gate_and_recovery(
        &counting,
        None,
        None,
        Some(&backend),
        None,
        None,
        "browser_click",
        r#"{"ref":"e1"}"#,
        &cancel,
        &gate,
        &tool_run,
        None,
        Some(&recovery),
    )
    .await
    .expect_err("mutation before snapshot denied");
    assert!(blocked.message().contains("browser_snapshot"));

    dispatch_agent_tool_with_gate_and_recovery(
        &counting,
        None,
        None,
        Some(&backend),
        None,
        None,
        "browser_snapshot",
        r#"{}"#,
        &cancel,
        &gate,
        &tool_run,
        None,
        Some(&recovery),
    )
    .await
    .expect("snapshot clears gate");

    dispatch_agent_tool_with_gate_and_recovery(
        &counting,
        None,
        None,
        Some(&backend),
        None,
        None,
        "browser_click",
        r#"{"ref":"e1"}"#,
        &cancel,
        &gate,
        &tool_run,
        None,
        Some(&recovery),
    )
    .await
    .expect("mutation after snapshot allowed");
}

#[tokio::test]
async fn host_restart_cancels_pending_interventions() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping host_restart_cancels_pending_interventions");
        return;
    };
    let owner = format!("owner-{}", Uuid::new_v4());
    let computer = insert_computer_placeholder(&pool, &owner, "restart")
        .await
        .unwrap();
    let (run_id, _) = seed_run(&pool, &owner, &computer.id).await;
    let service = HumanInterventionService {
        pool: pool.clone(),
        registry: Arc::new(cloud_host::approval::ApprovalWaitRegistry::default()),
    };
    let intervention_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO human_intervention_requests (
            id, run_id, owner_id, computer_id, reason, message, status, requested_at, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, 'consent', 'Accept cookies', 'pending', NOW(), NOW(), NOW())
        "#,
    )
    .bind(&intervention_id)
    .bind(&run_id)
    .bind(&owner)
    .bind(&computer.id)
    .execute(&pool)
    .await
    .unwrap();

    let count = service.cancel_all_pending_on_host_restart().await.unwrap();
    assert!(count >= 1, "expected at least one pending row cancelled");

    let status: String =
        sqlx::query_scalar("SELECT status FROM human_intervention_requests WHERE id = $1")
            .bind(&intervention_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "cancelled");
}

#[test]
fn message_bounding_rejects_secrets_in_payload() {
    use agent_core::sanitize_human_intervention_message;
    let long = "x".repeat(600);
    assert!(sanitize_human_intervention_message(&long).is_err());
    assert!(sanitize_human_intervention_message("  Please sign in  ").is_ok());
}
