//! ask_user lifecycle: same-run wait, isolation, bounds, restart, Slack notice.

use agent_core::{
    all_openai_tool_definitions, dispatch_agent_tool_with_gate_and_recovery, AgentComputer,
    AllowAllApprovalGate, ComputerError, ComputerInfo, CreateResponseResult, EventSink, ExecResult,
    ModelError, ResponsesModel, RunEventReceipt, RunStore, StructuredMessageInput, ToolRunContext,
    WorkspaceEntry, ASK_USER_TOOL_NAME,
};
use async_trait::async_trait;
use cloud_host::approval::ApprovalWaitRegistry;
use cloud_host::auth::{JwtVerifier, JwtVerifierConfig};
use cloud_host::config::{AuthMode, Config};
use cloud_host::db::resources::{insert_bot, insert_computer_placeholder};
use cloud_host::user_questions::UserQuestionService;
use cloud_host::{build_router, test_signing, AppState, TestRunOverrides};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::atomic::AtomicBool;
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
        computer: Arc<dyn AgentComputer>,
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

struct MockComputer;

#[async_trait]
impl AgentComputer for MockComputer {
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
    let config = Config {
        database_url: std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://elsewhere:elsewhere@127.0.0.1:5432/elsewhere".into()),
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

async fn seed_bot(pool: &PgPool, owner: &str) -> cloud_host::db::resources::BotRow {
    let computer = insert_computer_placeholder(pool, owner, "Computer")
        .await
        .unwrap();
    insert_bot(
        pool,
        owner,
        "Researcher",
        "Ask only when needed",
        "gpt-5.6-luna",
        Some(computer.id.as_str()),
        "responses",
        "sky-wisp",
    )
    .await
    .unwrap()
}

#[test]
fn catalog_exposes_ask_user_and_attachments() {
    let names: Vec<_> = all_openai_tool_definitions()
        .into_iter()
        .filter_map(|v| v.get("name").and_then(|n| n.as_str()).map(str::to_string))
        .collect();
    assert!(names.contains(&ASK_USER_TOOL_NAME.to_string()));
    assert!(names.contains(&"attachment_list".to_string()));
    assert!(names.contains(&"attachment_read".to_string()));
    // Subagents are tool-less in v1 and must not inherit owner-input tools.
    assert!(!agent_core::SUBAGENT_TOOL_NAMES.contains(&ASK_USER_TOOL_NAME));
    assert!(!agent_core::SUBAGENT_TOOL_NAMES.contains(&"attachment_list"));
    assert!(!agent_core::SUBAGENT_TOOL_NAMES.contains(&"attachment_read"));
}

#[sqlx::test(migrations = "./migrations")]
async fn same_turn_answer_continues_without_new_run(pool: PgPool) {
    let owner = format!("q-{}", Uuid::new_v4());
    let bot = seed_bot(&pool, &owner).await;
    let state = jwt_state(pool.clone());
    let app = build_router(state.clone());
    let request_id = Uuid::new_v4().to_string();
    let model = Arc::new(ScriptedModel {
        steps: Mutex::new(vec![
            CreateResponseResult {
                output: vec![json!({
                    "type": "function_call",
                    "name": "ask_user",
                    "call_id": "ask-1",
                    "arguments": "{\"question\":\"Which environment should I deploy to?\",\"options\":[\"Staging\",\"Production\",\"Don't deploy\"]}"
                })],
                output_text: None,
            },
            CreateResponseResult {
                output: vec![json!({
                    "type": "message",
                    "content": [{"type":"output_text","text":"Deploying to Staging"}]
                })],
                output_text: Some("Deploying to Staging".into()),
            },
        ]),
    });
    let _guard = RunOverrideGuard::install(
        state.clone(),
        request_id.clone(),
        Arc::new(MockComputer),
        model,
    );
    let create = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/runs")
                .header("Authorization", format!("Bearer {}", token(&owner)))
                .header("Idempotency-Key", &request_id)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({ "botId": bot.id, "message": "Deploy when ready" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), axum::http::StatusCode::ACCEPTED);
    let created: Value =
        serde_json::from_slice(&create.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let run_id = created["runId"].as_str().unwrap().to_string();
    cloud_host::worker::dispatch_available(&state)
        .await
        .unwrap();

    let mut question_id = String::new();
    for _ in 0..80 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let pending: Option<String> = sqlx::query_scalar(
            "SELECT id FROM run_user_questions WHERE run_id = $1 AND status = 'pending'",
        )
        .bind(&run_id)
        .fetch_optional(&pool)
        .await
        .unwrap();
        if let Some(id) = pending {
            question_id = id;
            break;
        }
    }
    assert!(!question_id.is_empty(), "timed out waiting for ask_user");

    let runs_before: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs WHERE owner_id = $1")
            .bind(&owner)
            .fetch_one(&pool)
            .await
            .unwrap();
    let queue_before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM work_queue q JOIN agent_runs r ON r.id = q.run_id WHERE r.owner_id = $1",
    )
            .bind(&owner)
            .fetch_one(&pool)
            .await
            .unwrap();

    let answer = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/v1/runs/{run_id}/questions/{question_id}/answer"))
                .header("Authorization", format!("Bearer {}", token(&owner)))
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({ "selectedIndex": 0 }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(answer.status(), axum::http::StatusCode::OK);

    let second = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/v1/runs/{run_id}/questions/{question_id}/answer"))
                .header("Authorization", format!("Bearer {}", token(&owner)))
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({ "selectedIndex": 1 }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        second.status() == axum::http::StatusCode::OK
            || second.status() == axum::http::StatusCode::CONFLICT
            || second.status() == axum::http::StatusCode::BAD_REQUEST
    );
    let answered: Value =
        serde_json::from_slice(&answer.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(answered["selectedIndex"], 0);

    for _ in 0..80 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let status: String = sqlx::query_scalar("SELECT status FROM agent_runs WHERE id = $1")
            .bind(&run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        if status == "completed" {
            break;
        }
    }
    let body: String = sqlx::query_scalar(
        "SELECT COALESCE(m.body, '') FROM agent_runs r LEFT JOIN messages m ON m.id = r.assistant_message_id WHERE r.id = $1",
    )
    .bind(&run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        body.contains("Staging"),
        "assistant should use the chosen option: {body}"
    );

    let runs_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs WHERE owner_id = $1")
        .bind(&owner)
        .fetch_one(&pool)
        .await
        .unwrap();
    let queue_after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM work_queue q JOIN agent_runs r ON r.id = q.run_id WHERE r.owner_id = $1",
    )
            .bind(&owner)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(runs_before, runs_after);
    assert_eq!(queue_before, queue_after);

    let foreign = app
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri(format!("/v1/runs/{run_id}/question"))
                .header("Authorization", format!("Bearer {}", token("other")))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(foreign.status(), axum::http::StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn one_pending_max_three_and_duplicate_invocation(pool: PgPool) {
    let owner = format!("lim-{}", Uuid::new_v4());
    let bot = seed_bot(&pool, &owner).await;
    let records = cloud_host::work::enqueue(
        &pool,
        &owner,
        &Uuid::new_v4().to_string(),
        &bot.id,
        None,
        "decide",
    )
    .await
    .unwrap();
    sqlx::query("UPDATE agent_runs SET status = 'running' WHERE id = $1")
        .bind(&records.run_id)
        .execute(&pool)
        .await
        .unwrap();
    let service = UserQuestionService {
        pool: pool.clone(),
        registry: Arc::new(ApprovalWaitRegistry::default()),
    };
    struct NoopStore;
    #[async_trait]
    impl RunStore for NoopStore {
        async fn append_run_event(
            &self,
            _request_id: &str,
            _event_type: &str,
            _payload: &Value,
        ) -> Result<RunEventReceipt, agent_core::RuntimeError> {
            Ok(RunEventReceipt { id: 1 })
        }
        async fn create_run(
            &self,
            _params: agent_core::CreateRunParams,
        ) -> Result<String, agent_core::RuntimeError> {
            Ok("x".into())
        }
        async fn persist_structured_message(
            &self,
            _input: StructuredMessageInput,
        ) -> Result<agent_core::PersistedMessage, agent_core::RuntimeError> {
            Err(agent_core::RuntimeError::Store("unused".into()))
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
    }
    let store = Arc::new(NoopStore);
    let events = Arc::new(NoopEvents);
    let cancel = AtomicBool::new(false);
    let scoped = cloud_host::user_questions::RunScopedUserQuestion::new(
        service.clone(),
        store,
        events,
        Arc::new(AtomicBool::new(false)),
    );
    let run = ToolRunContext {
        run_id: records.run_id.clone(),
        request_id: records.request_id.clone(),
        owner_id: owner.clone(),
        bot_id: bot.id.clone(),
        computer_id: records.computer_id.clone(),
        tool_invocation_id: Some("inv-1".into()),
    };
    let first = tokio::spawn({
        let scoped = scoped.clone();
        let run = run.clone();
        async move {
            dispatch_agent_tool_with_gate_and_recovery(
                &MockComputer,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(&scoped),
                "ask_user",
                r#"{"question":"Pick one?","options":["A","B"]}"#,
                &cancel,
                &AllowAllApprovalGate,
                &run,
                None,
                None,
            )
            .await
        }
    });
    tokio::time::sleep(Duration::from_millis(80)).await;
    let pending = service
        .get_pending_for_owner_run(&owner, &records.run_id)
        .await
        .unwrap();
    assert!(pending.is_some());
    let duplicate = dispatch_agent_tool_with_gate_and_recovery(
        &MockComputer,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(&scoped),
        "ask_user",
        r#"{"question":"Pick one?","options":["A","B"]}"#,
        &AtomicBool::new(false),
        &AllowAllApprovalGate,
        &ToolRunContext {
            tool_invocation_id: Some("inv-2".into()),
            ..run.clone()
        },
        None,
        None,
    )
    .await;
    assert!(duplicate.is_err());
    first.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn host_restart_interrupts_pending_questions(pool: PgPool) {
    let owner = format!("rst-{}", Uuid::new_v4());
    let bot = seed_bot(&pool, &owner).await;
    let records = cloud_host::work::enqueue(
        &pool,
        &owner,
        &Uuid::new_v4().to_string(),
        &bot.id,
        None,
        "wait",
    )
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO run_user_questions (
            id, owner_id, run_id, request_id, tool_invocation_id, question, options, status, requested_at, updated_at
        ) VALUES ($1,$2,$3,$4,'inv','Which?', '["A","B"]'::jsonb, 'pending', NOW(), NOW())
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&owner)
    .bind(&records.run_id)
    .bind(&records.request_id)
    .execute(&pool)
    .await
    .unwrap();
    let service = UserQuestionService {
        pool: pool.clone(),
        registry: Arc::new(ApprovalWaitRegistry::default()),
    };
    let n = service
        .interrupt_all_pending_on_host_restart()
        .await
        .unwrap();
    assert!(n >= 1);
    let status: String =
        sqlx::query_scalar("SELECT status FROM run_user_questions WHERE run_id = $1")
            .bind(&records.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "interrupted");
}

#[sqlx::test(migrations = "./migrations")]
async fn slack_origin_enqueues_choice_attention(pool: PgPool) {
    let owner = format!("sl-{}", Uuid::new_v4());
    let bot = seed_bot(&pool, &owner).await;
    let records = cloud_host::work::enqueue(
        &pool,
        &owner,
        &Uuid::new_v4().to_string(),
        &bot.id,
        None,
        "from slack",
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE agent_runs SET origin_kind = 'channel', origin_provider = 'slack' WHERE id = $1",
    )
    .bind(&records.run_id)
    .execute(&pool)
    .await
    .unwrap();
    let sent = cloud_host::channels::delivery::enqueue_owner_attention_with_detail(
        &pool,
        &records.run_id,
        "choice",
        Some("Which environment should I deploy to?\nStaging / Production"),
        Some("https://app.elsewhere.test"),
    )
    .await
    .unwrap();
    // Without a Slack thread this is a no-op, which is still owner-safe.
    assert!(!sent);
}
