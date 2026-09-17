//! Phase 3C.2 approval enforcement tests (requires Postgres).

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
        state.register_test_run_overrides(&request_id, TestRunOverrides { computer, model });
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

fn jwt_state(pool: PgPool) -> AppState {
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
        tool_approval_timeout_secs: 300,
        enforce_tool_approvals_internal: true,
        legacy_local_approval_bypass: false,
        browser_enabled: false,
        connector_secret_key: None,
        github_client_id: None,
        github_client_secret: None,
        github_oauth_redirect_uri: None,
        slack_client_id: None,
        slack_client_secret: None,
        slack_signing_secret: None,
        slack_oauth_redirect_uri: None,
        slack_api_base: "https://slack.com/api".into(),
        local_mac_credential_key: None,
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

#[sqlx::test(migrations = "./migrations")]
async fn read_tools_auto_allowed_without_approval_row(pool: PgPool) {
    let computer = Arc::new(CountingComputer {
        writes: AtomicUsize::new(0),
        execs: AtomicUsize::new(0),
    });
    let model = Arc::new(ScriptedModel {
        steps: Mutex::new(vec![
            CreateResponseResult {
                output: vec![json!({
                    "type": "function_call",
                    "name": "workspace_read",
                    "call_id": "c1",
                    "arguments": "{\"path\":\"/workspace/a\"}"
                })],
                output_text: None,
            },
            CreateResponseResult {
                output: vec![json!({
                    "type": "message",
                    "content": [{"type":"output_text","text":"ok"}]
                })],
                output_text: Some("ok".into()),
            },
        ]),
    });
    let owner = format!("user-a-read-{}", Uuid::new_v4());
    let state = jwt_state(pool.clone());
    let request_id = Uuid::new_v4().to_string();
    let _run_guard =
        RunOverrideGuard::install(state.clone(), request_id.clone(), computer.clone(), model);
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

    let app = build_router(state.clone());
    let body = json!({
        "botId": bot.id,
        "message": "read"
    });
    let resp = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/runs")
                .header("Authorization", format!("Bearer {}", token(&owner)))
                .header("Idempotency-Key", &request_id)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::ACCEPTED);
    cloud_host::worker::dispatch_available(&state)
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(2)).await;
    let pending: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM tool_approval_requests WHERE owner_id = $1 AND status = 'pending'",
    )
    .bind(&owner)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pending.0, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn write_waits_for_approval_before_computer_call(pool: PgPool) {
    let computer = Arc::new(CountingComputer {
        writes: AtomicUsize::new(0),
        execs: AtomicUsize::new(0),
    });
    let model = Arc::new(ScriptedModel {
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
    });
    let owner = format!("user-a-write-{}", Uuid::new_v4());
    let state = jwt_state(pool.clone());
    let request_id = Uuid::new_v4().to_string();
    let _run_guard =
        RunOverrideGuard::install(state.clone(), request_id.clone(), computer.clone(), model);
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

    let app = build_router(state.clone());
    let body = json!({ "botId": bot.id, "message": "write" });
    let _ = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/runs")
                .header("Authorization", format!("Bearer {}", token(&owner)))
                .header("Idempotency-Key", &request_id)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    cloud_host::worker::dispatch_available(&state)
        .await
        .unwrap();

    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if computer.writes.load(Ordering::SeqCst) == 0 {
            let pending: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM tool_approval_requests WHERE owner_id = $1 AND status = 'pending'",
            )
            .bind(&owner)
            .fetch_one(&pool)
            .await
            .unwrap();
            if pending.0 >= 1 {
                assert_eq!(computer.writes.load(Ordering::SeqCst), 0);
                return;
            }
        }
    }
    panic!("expected pending approval without write");
}

#[sqlx::test(migrations = "./migrations")]
async fn user_b_cannot_resolve_user_a_approval(pool: PgPool) {
    let owner_a = format!("owner-a-{}", Uuid::new_v4());
    let owner_b = format!("owner-b-{}", Uuid::new_v4());
    let approval_id = Uuid::new_v4().to_string();
    let run_id = Uuid::new_v4().to_string();
    let computer_row = insert_computer_placeholder(&pool, &owner_a, "c")
        .await
        .unwrap();
    let bot = insert_bot(
        &pool,
        &owner_a,
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
    .bind(&owner_a)
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
    .bind(&owner_a)
    .bind(Uuid::new_v4().to_string())
    .bind(&bot.id)
    .bind(&conv_id)
    .bind(&computer_row.id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO tool_approval_requests (id, run_id, owner_id, tool_name, tool_kind, arguments_json, status)
        VALUES ($1, $2, $3, 'workspace_write', 'mutation', '{}', 'pending')
        "#,
    )
    .bind(&approval_id)
    .bind(&run_id)
    .bind(&owner_a)
    .execute(&pool)
    .await
    .unwrap();

    let app = build_router(jwt_state(pool));
    let resp = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/v1/approvals/{approval_id}/approve"))
                .header("Authorization", format!("Bearer {}", token(&owner_b)))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::NOT_FOUND);
}
