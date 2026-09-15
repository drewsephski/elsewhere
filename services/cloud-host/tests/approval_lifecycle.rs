//! Approval lifecycle, races, SSE ordering, and host-restart recovery.

use agent_core::{
    AgentComputer, ComputerError, ComputerInfo, CreateResponseResult, ExecResult, ModelError,
    ResponsesModel, WorkspaceEntry,
};
use async_trait::async_trait;
use cloud_host::auth::{JwtVerifier, JwtVerifierConfig};
use cloud_host::config::{AuthMode, Config};
use cloud_host::db::resources::{insert_bot, insert_computer_placeholder};
use cloud_host::{build_router, test_signing, AppState, TestRunOverrides};
use serde_json::json;
use sqlx::PgPool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tower::ServiceExt;
use uuid::Uuid;

const TEST_JWT_ISSUER: &str = "http://localhost:3000";
const TEST_JWT_AUDIENCE: &str = "elsewhere-cloud-host";

/// Clears injected run dependencies when the test finishes (including on panic).
struct RunOverrideGuard {
    state: AppState,
    request_id: String,
}

impl RunOverrideGuard {
    fn install(
        state: AppState,
        request_id: String,
        computer: Arc<CountingComputer>,
        model: Arc<dyn ResponsesModel>,
    ) -> Self {
        state.register_test_run_overrides(
            &request_id,
            TestRunOverrides {
                computer,
                model,
            },
        );
        Self { state, request_id }
    }
}

impl Drop for RunOverrideGuard {
    fn drop(&mut self) {
        self.state.clear_test_run_overrides(&self.request_id);
    }
}

struct CountingComputer {
    writes: AtomicUsize,
    execs: AtomicUsize,
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
        Ok(b"ok".to_vec())
    }

    async fn write_file(&self, _path: &str, _data: &[u8]) -> Result<(), ComputerError> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn exec(&self, _command: &str) -> Result<ExecResult, ComputerError> {
        self.execs.fetch_add(1, Ordering::SeqCst);
        Ok(ExecResult {
            ok: true,
            stdout: String::new(),
            stderr: String::new(),
            exit_code: 0,
        })
    }
}

struct ScriptedModel {
    steps: Mutex<Vec<CreateResponseResult>>,
}

#[async_trait]
impl ResponsesModel for ScriptedModel {
    async fn create_response(
        &self,
        _request: agent_core::CreateResponseRequest,
    ) -> Result<CreateResponseResult, ModelError> {
        let mut steps = self.steps.lock().unwrap();
        if steps.is_empty() {
            return Ok(CreateResponseResult {
                output: vec![json!({
                    "type": "message",
                    "content": [{"type":"output_text","text":"done"}]
                })],
                output_text: Some("done".into()),
            });
        }
        Ok(steps.remove(0))
    }
}

fn jwt_state(pool: PgPool, approval_timeout_secs: u64) -> AppState {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://elsewhere:elsewhere@127.0.0.1:5432/elsewhere".into());
    let config = Config {
        database_url,
        openai_api_key: Some("test-key".into()),
        sprite_token: "test-sprite".into(),
        api_token: "test-token".into(),
        auth_mode: AuthMode::Jwt,
        jwt_issuer: Some(TEST_JWT_ISSUER.into()),
        jwt_audience: Some(TEST_JWT_AUDIENCE.into()),
        jwt_jwks_url: Some("http://127.0.0.1:9/jwks".into()),
        cors_web_origin: None,
        allow_codex_login: false,
        sprites_api_base: "http://127.0.0.1:9".into(),
        max_concurrent_runs: 4,
        run_timeout_secs: 120,
        bind_addr: "127.0.0.1:0".into(),
        run_engine: cloud_host::run_engine_select::RunEngineMode::Responses,
        codex_executable: None,
        codex_profiles_dir: None,
        browser_profiles_dir: None,
        tool_approval_timeout_secs: approval_timeout_secs,
        enforce_tool_approvals_internal: true,
        legacy_local_approval_bypass: false,
        browser_enabled: false,
        connector_secret_key: None,
        github_client_id: None,
        github_client_secret: None,
        github_oauth_redirect_uri: None,
    };
    let mut state = AppState::new(pool, config);
    state.jwt_verifier = Some(JwtVerifier::from_test_decoding_key(
        test_signing::TEST_KID,
        test_signing::verifier(),
        JwtVerifierConfig {
            jwks_url: "http://127.0.0.1:9/jwks".into(),
            issuer: TEST_JWT_ISSUER.into(),
            audience: TEST_JWT_AUDIENCE.into(),
        },
    ));
    state
}

fn token(sub: &str) -> String {
    test_signing::user_token(sub, TEST_JWT_ISSUER, TEST_JWT_AUDIENCE, 300)
}

async fn setup_bot(pool: &PgPool, owner: &str) -> cloud_host::db::resources::BotRow {
    let computer_row = insert_computer_placeholder(pool, owner, "c").await.unwrap();
    insert_bot(
        pool,
        owner,
        "b",
        "i",
        "gpt-5.6-luna",
        Some(computer_row.id.as_str()),
        "responses",
        "sky-wisp",
    )
    .await
    .unwrap()
}

async fn start_run(
    app: &axum::Router,
    state: &AppState,
    owner: &str,
    bot_id: &str,
    request_id: &str,
    message: &str,
    model: Arc<dyn ResponsesModel>,
    computer: Arc<CountingComputer>,
) -> RunOverrideGuard {
    let guard = RunOverrideGuard::install(
        state.clone(),
        request_id.to_string(),
        computer,
        model,
    );
    let body = json!({ "botId": bot_id, "message": message });
    let resp = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/runs")
                .header("Authorization", format!("Bearer {}", token(owner)))
                .header("Idempotency-Key", request_id)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::ACCEPTED);
    cloud_host::worker::dispatch_available(state).await.unwrap();
    guard
}

async fn wait_pending_approval_id(pool: &PgPool, owner: &str) -> String {
    for _ in 0..80 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM tool_approval_requests WHERE owner_id = $1 AND status = 'pending' ORDER BY requested_at DESC LIMIT 1",
        )
        .bind(owner)
        .fetch_optional(pool)
        .await
        .unwrap();
        if let Some((id,)) = row {
            return id;
        }
    }
    panic!("timed out waiting for pending approval");
}

async fn count_resolved_events(pool: &PgPool, request_id: &str) -> i64 {
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM run_events WHERE request_id = $1 AND event_type = 'approval_resolved'",
    )
    .bind(request_id)
    .fetch_one(pool)
    .await
    .unwrap();
    count
}

fn write_then_done_model() -> Arc<ScriptedModel> {
    Arc::new(ScriptedModel {
        steps: Mutex::new(vec![
            CreateResponseResult {
                output: vec![json!({
                    "type": "function_call",
                    "name": "workspace_write",
                    "call_id": "c1",
                    "arguments": "{\"path\":\"/workspace/a.txt\",\"content\":\"hi\"}"
                })],
                output_text: None,
            },
            CreateResponseResult {
                output: vec![json!({
                    "type": "message",
                    "content": [{"type":"output_text","text":"done"}]
                })],
                output_text: Some("done".into()),
            },
        ]),
    })
}

fn exec_then_done_model() -> Arc<ScriptedModel> {
    Arc::new(ScriptedModel {
        steps: Mutex::new(vec![
            CreateResponseResult {
                output: vec![json!({
                    "type": "function_call",
                    "name": "workspace_exec",
                    "call_id": "c1",
                    "arguments": "{\"command\":\"echo hi\"}"
                })],
                output_text: None,
            },
            CreateResponseResult {
                output: vec![json!({
                    "type": "message",
                    "content": [{"type":"output_text","text":"done"}]
                })],
                output_text: Some("done".into()),
            },
        ]),
    })
}

#[sqlx::test(migrations = "./migrations")]
async fn approve_executes_write_once(pool: PgPool) {
    let owner = format!("appr-write-{}", Uuid::new_v4());
    let bot = setup_bot(&pool, &owner).await;
    let computer = Arc::new(CountingComputer {
        writes: AtomicUsize::new(0),
        execs: AtomicUsize::new(0),
    });
    let state = jwt_state(pool.clone(), 300);
    let app = build_router(state.clone());
    let request_id = Uuid::new_v4().to_string();
    let _run_guard = start_run(
        &app,
        &state,
        &owner,
        &bot.id,
        &request_id,
        "write",
        write_then_done_model(),
        computer.clone(),
    )
    .await;

    let approval_id = wait_pending_approval_id(&pool, &owner).await;
    let approve = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/v1/approvals/{approval_id}/approve"))
                .header("Authorization", format!("Bearer {}", token(&owner)))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(approve.status(), axum::http::StatusCode::OK);

    for _ in 0..60 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if computer.writes.load(Ordering::SeqCst) == 1 {
            break;
        }
    }
    assert_eq!(computer.writes.load(Ordering::SeqCst), 1);
    assert_eq!(count_resolved_events(&pool, &request_id).await, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn fast_immediate_approval_does_not_lose_wakeup(pool: PgPool) {
    let owner = format!("fast-appr-{}", Uuid::new_v4());
    let bot = setup_bot(&pool, &owner).await;
    let computer = Arc::new(CountingComputer {
        writes: AtomicUsize::new(0),
        execs: AtomicUsize::new(0),
    });
    let state = jwt_state(pool.clone(), 300);
    let app = build_router(state.clone());
    let request_id = Uuid::new_v4().to_string();
    let _run_guard = start_run(
        &app,
        &state,
        &owner,
        &bot.id,
        &request_id,
        "write",
        write_then_done_model(),
        computer.clone(),
    )
    .await;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    let approval_id = loop {
        if tokio::time::Instant::now() >= deadline {
            let pending: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM tool_approval_requests WHERE owner_id = $1 AND status = 'pending'",
            )
            .bind(&owner)
            .fetch_one(&pool)
            .await
            .unwrap_or((0,));
            let writes = computer.writes.load(Ordering::SeqCst);
            panic!(
                "timed out waiting for pending approval to approve immediately \
                 (owner={owner}, request_id={request_id}, pending_count={}, writes={})",
                pending.0,
                writes
            );
        }
        if let Ok(Some((id,))) = sqlx::query_as::<_, (String,)>(
            "SELECT id FROM tool_approval_requests WHERE owner_id = $1 AND status = 'pending' ORDER BY requested_at DESC LIMIT 1",
        )
        .bind(&owner)
        .fetch_optional(&pool)
        .await
        {
            let approve = app
                .clone()
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri(format!("/v1/approvals/{id}/approve"))
                        .header("Authorization", format!("Bearer {}", token(&owner)))
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(approve.status(), axum::http::StatusCode::OK);
            break id;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    };

    for _ in 0..80 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if computer.writes.load(Ordering::SeqCst) == 1 {
            return;
        }
    }
    panic!("write never executed after immediate approve (approval_id={approval_id})");
}

#[sqlx::test(migrations = "./migrations")]
async fn deny_executes_zero_writes(pool: PgPool) {
    let owner = format!("deny-write-{}", Uuid::new_v4());
    let bot = setup_bot(&pool, &owner).await;
    let computer = Arc::new(CountingComputer {
        writes: AtomicUsize::new(0),
        execs: AtomicUsize::new(0),
    });
    let state = jwt_state(pool.clone(), 300);
    let app = build_router(state.clone());
    let request_id = Uuid::new_v4().to_string();
    let _run_guard = start_run(
        &app,
        &state,
        &owner,
        &bot.id,
        &request_id,
        "write",
        write_then_done_model(),
        computer.clone(),
    )
    .await;

    let approval_id = wait_pending_approval_id(&pool, &owner).await;
    let deny = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/v1/approvals/{approval_id}/deny"))
                .header("Authorization", format!("Bearer {}", token(&owner)))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deny.status(), axum::http::StatusCode::OK);

    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_eq!(computer.writes.load(Ordering::SeqCst), 0);
    assert_eq!(count_resolved_events(&pool, &request_id).await, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn exec_approval_required_like_write(pool: PgPool) {
    let owner = format!("exec-appr-{}", Uuid::new_v4());
    let bot = setup_bot(&pool, &owner).await;
    let computer = Arc::new(CountingComputer {
        writes: AtomicUsize::new(0),
        execs: AtomicUsize::new(0),
    });
    let state = jwt_state(pool.clone(), 300);
    let app = build_router(state.clone());
    let _run_guard = start_run(
        &app,
        &state,
        &owner,
        &bot.id,
        &Uuid::new_v4().to_string(),
        "exec",
        exec_then_done_model(),
        computer.clone(),
    )
    .await;

    let approval_id = wait_pending_approval_id(&pool, &owner).await;
    assert_eq!(computer.execs.load(Ordering::SeqCst), 0);

    let approve = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/v1/approvals/{approval_id}/approve"))
                .header("Authorization", format!("Bearer {}", token(&owner)))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(approve.status(), axum::http::StatusCode::OK);

    for _ in 0..60 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if computer.execs.load(Ordering::SeqCst) == 1 {
            return;
        }
    }
    panic!("exec never ran after approval");
}

#[sqlx::test(migrations = "./migrations")]
async fn timeout_executes_zero_writes(pool: PgPool) {
    let owner = format!("timeout-{}", Uuid::new_v4());
    let bot = setup_bot(&pool, &owner).await;
    let computer = Arc::new(CountingComputer {
        writes: AtomicUsize::new(0),
        execs: AtomicUsize::new(0),
    });
    let state = jwt_state(pool.clone(), 2);
    let app = build_router(state.clone());
    let request_id = Uuid::new_v4().to_string();
    let _run_guard = start_run(
        &app,
        &state,
        &owner,
        &bot.id,
        &request_id,
        "write",
        write_then_done_model(),
        computer.clone(),
    )
    .await;

    let _ = wait_pending_approval_id(&pool, &owner).await;
    tokio::time::sleep(Duration::from_secs(4)).await;
    assert_eq!(computer.writes.load(Ordering::SeqCst), 0);
    let status: (String,) = sqlx::query_as(
        "SELECT status FROM tool_approval_requests WHERE owner_id = $1 ORDER BY requested_at DESC LIMIT 1",
    )
    .bind(&owner)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status.0, "expired");
    assert_eq!(count_resolved_events(&pool, &request_id).await, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn cancel_while_pending_executes_zero_writes(pool: PgPool) {
    let owner = format!("cancel-run-{}", Uuid::new_v4());
    let bot = setup_bot(&pool, &owner).await;
    let computer = Arc::new(CountingComputer {
        writes: AtomicUsize::new(0),
        execs: AtomicUsize::new(0),
    });
    let state = jwt_state(pool.clone(), 300);
    let app = build_router(state.clone());
    let request_id = Uuid::new_v4().to_string();
    let _run_guard = start_run(
        &app,
        &state,
        &owner,
        &bot.id,
        &request_id,
        "write",
        write_then_done_model(),
        computer.clone(),
    )
    .await;

    let approval_id = wait_pending_approval_id(&pool, &owner).await;
    let run_id: (String,) =
        sqlx::query_as("SELECT run_id FROM tool_approval_requests WHERE id = $1")
            .bind(&approval_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    let cancel = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/v1/runs/{}/cancel", run_id.0))
                .header("Authorization", format!("Bearer {}", token(&owner)))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cancel.status(), axum::http::StatusCode::ACCEPTED);

    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_eq!(computer.writes.load(Ordering::SeqCst), 0);
    assert_eq!(count_resolved_events(&pool, &request_id).await, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn double_approve_single_winner_and_one_resolved_event(pool: PgPool) {
    let owner = format!("dbl-appr-{}", Uuid::new_v4());
    let bot = setup_bot(&pool, &owner).await;
    let computer = Arc::new(CountingComputer {
        writes: AtomicUsize::new(0),
        execs: AtomicUsize::new(0),
    });
    let state = jwt_state(pool.clone(), 300);
    let app = build_router(state.clone());
    let request_id = Uuid::new_v4().to_string();
    let _run_guard = start_run(
        &app,
        &state,
        &owner,
        &bot.id,
        &request_id,
        "write",
        write_then_done_model(),
        computer.clone(),
    )
    .await;

    let approval_id = wait_pending_approval_id(&pool, &owner).await;
    let uri = format!("/v1/approvals/{approval_id}/approve");
    let auth = format!("Bearer {}", token(&owner));
    let first = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(&uri)
                .header("Authorization", &auth)
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), axum::http::StatusCode::OK);

    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if count_resolved_events(&pool, &request_id).await >= 1 {
            break;
        }
    }

    let second = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(&uri)
                .header("Authorization", &auth)
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), axum::http::StatusCode::NOT_FOUND);
    assert_eq!(count_resolved_events(&pool, &request_id).await, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn approve_vs_deny_race_has_single_terminal_status(pool: PgPool) {
    let owner = format!("race-ad-{}", Uuid::new_v4());
    let bot = setup_bot(&pool, &owner).await;
    let computer = Arc::new(CountingComputer {
        writes: AtomicUsize::new(0),
        execs: AtomicUsize::new(0),
    });
    let state = jwt_state(pool.clone(), 300);
    let app = build_router(state.clone());
    let _run_guard = start_run(
        &app,
        &state,
        &owner,
        &bot.id,
        &Uuid::new_v4().to_string(),
        "write",
        write_then_done_model(),
        computer.clone(),
    )
    .await;

    let approval_id = wait_pending_approval_id(&pool, &owner).await;
    let uri_a = format!("/v1/approvals/{approval_id}/approve");
    let uri_d = format!("/v1/approvals/{approval_id}/deny");
    let auth = format!("Bearer {}", token(&owner));

    let t1 = tokio::spawn({
        let app = app.clone();
        let uri_a = uri_a.clone();
        let auth = auth.clone();
        async move {
            app.oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri(uri_a)
                    .header("Authorization", auth)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
        }
    });
    let t2 = tokio::spawn({
        let app = app.clone();
        async move {
            app.oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri(uri_d)
                    .header("Authorization", auth)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
        }
    });

    let _ = tokio::join!(t1, t2);
    let status: (String,) =
        sqlx::query_as("SELECT status FROM tool_approval_requests WHERE id = $1")
            .bind(&approval_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(status.0 == "approved" || status.0 == "denied");
    assert!(computer.writes.load(Ordering::SeqCst) <= 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn late_approve_after_expiry_does_not_execute(pool: PgPool) {
    let owner = format!("late-expiry-{}", Uuid::new_v4());
    let approval_id = Uuid::new_v4().to_string();
    let run_id = Uuid::new_v4().to_string();
    let computer_row = insert_computer_placeholder(&pool, &owner, "c")
        .await
        .unwrap();
    let bot = insert_bot(
        &pool,
        &owner,
        "b",
        "i",
        "gpt-5.6-luna",
        Some(computer_row.id.as_str()),
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
    .bind(&owner)
    .bind(&bot.id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO agent_runs (id, owner_id, request_id, bot_id, conversation_id, computer_id, model, status, step_count, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, 'gpt-5.6-luna', 'running', 0, NOW(), NOW())
        "#,
    )
    .bind(&run_id)
    .bind(&owner)
    .bind(Uuid::new_v4().to_string())
    .bind(&bot.id)
    .bind(&conv_id)
    .bind(&computer_row.id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO tool_approval_requests (
            id, run_id, owner_id, tool_name, tool_kind, arguments_json, status, expires_at, requested_at, created_at, updated_at
        ) VALUES ($1, $2, $3, 'workspace_write', 'mutation', '{}', 'pending', NOW() - INTERVAL '1 minute', NOW(), NOW(), NOW())
        "#,
    )
    .bind(&approval_id)
    .bind(&run_id)
    .bind(&owner)
    .execute(&pool)
    .await
    .unwrap();

    let state = jwt_state(pool.clone(), 300);
    let ok = state
        .approvals
        .approve(&owner, &approval_id, &owner)
        .await
        .unwrap();
    assert!(!ok);
    let status: (String,) =
        sqlx::query_as("SELECT status FROM tool_approval_requests WHERE id = $1")
            .bind(&approval_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status.0, "expired");
}

#[sqlx::test(migrations = "./migrations")]
async fn host_restart_cancels_stale_pending(pool: PgPool) {
    let owner = format!("host-restart-{}", Uuid::new_v4());
    let approval_id = Uuid::new_v4().to_string();
    let run_id = Uuid::new_v4().to_string();
    let request_id = Uuid::new_v4().to_string();
    let computer_row = insert_computer_placeholder(&pool, &owner, "c")
        .await
        .unwrap();
    let bot = insert_bot(
        &pool,
        &owner,
        "b",
        "i",
        "gpt-5.6-luna",
        Some(computer_row.id.as_str()),
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
    .bind(&owner)
    .bind(&bot.id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO agent_runs (id, owner_id, request_id, bot_id, conversation_id, computer_id, model, status, step_count, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, 'gpt-5.6-luna', 'running', 0, NOW(), NOW())
        "#,
    )
    .bind(&run_id)
    .bind(&owner)
    .bind(&request_id)
    .bind(&bot.id)
    .bind(&conv_id)
    .bind(&computer_row.id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO tool_approval_requests (id, run_id, owner_id, tool_name, tool_kind, arguments_json, status, requested_at, created_at, updated_at)
        VALUES ($1, $2, $3, 'workspace_write', 'mutation', '{}', 'pending', NOW(), NOW(), NOW())
        "#,
    )
    .bind(&approval_id)
    .bind(&run_id)
    .bind(&owner)
    .execute(&pool)
    .await
    .unwrap();

    let state = jwt_state(pool.clone(), 300);
    let _ = state
        .approvals
        .cancel_all_pending_on_host_restart()
        .await
        .unwrap();

    let row: (String, Option<String>) = sqlx::query_as(
        "SELECT status, resolution_reason FROM tool_approval_requests WHERE id = $1",
    )
    .bind(&approval_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, "cancelled");
    assert_eq!(row.1.as_deref(), Some("host_restart"));

    let revived = state
        .approvals
        .approve(&owner, &approval_id, &owner)
        .await
        .unwrap();
    assert!(!revived);

    let audit: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM run_events WHERE request_id = $1 AND event_type = 'approval_resolved'",
    )
    .bind(&request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit.0, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn approval_sse_events_ordered_and_replay_once(pool: PgPool) {
    let owner = format!("sse-appr-{}", Uuid::new_v4());
    let bot = setup_bot(&pool, &owner).await;
    let computer = Arc::new(CountingComputer {
        writes: AtomicUsize::new(0),
        execs: AtomicUsize::new(0),
    });
    let state = jwt_state(pool.clone(), 300);
    let app = build_router(state.clone());
    let request_id = Uuid::new_v4().to_string();
    let _run_guard = start_run(
        &app,
        &state,
        &owner,
        &bot.id,
        &request_id,
        "write",
        write_then_done_model(),
        computer.clone(),
    )
    .await;

    let approval_id = wait_pending_approval_id(&pool, &owner).await;
    let run_id: (String,) =
        sqlx::query_as("SELECT run_id FROM tool_approval_requests WHERE id = $1")
            .bind(&approval_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    let _ = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/v1/approvals/{approval_id}/approve"))
                .header("Authorization", format!("Bearer {}", token(&owner)))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    for _ in 0..60 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if count_resolved_events(&pool, &request_id).await >= 1 {
            break;
        }
    }

    let events: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, event_type FROM run_events WHERE request_id = $1 ORDER BY id ASC",
    )
    .bind(&request_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    let types = events.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>();
    let req_pos = types
        .iter()
        .position(|t| *t == "approval_requested")
        .unwrap();
    let res_pos = types
        .iter()
        .position(|t| *t == "approval_resolved")
        .unwrap();
    assert!(req_pos < res_pos);
    assert_eq!(
        types.iter().filter(|t| **t == "approval_requested").count(),
        1
    );
    assert_eq!(
        types.iter().filter(|t| **t == "approval_resolved").count(),
        1
    );

    let cursor_before = events[req_pos].0 - 1;
    let sse = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/v1/runs/{}/events", run_id.0))
                .header("Authorization", format!("Bearer {}", token(&owner)))
                .header("Last-Event-ID", cursor_before.to_string())
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(sse.status(), axum::http::StatusCode::OK);

    let cursor_after = events[res_pos].0;
    let sse2 = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/v1/runs/{}/events", run_id.0))
                .header("Authorization", format!("Bearer {}", token(&owner)))
                .header("Last-Event-ID", cursor_after.to_string())
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(sse2.status(), axum::http::StatusCode::OK);

}
