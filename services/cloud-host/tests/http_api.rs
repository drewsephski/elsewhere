use agent_core::{
    AgentComputer, ComputerError, ComputerInfo, CreateResponseResult, ExecResult, ModelError,
    ResponsesModel, WorkspaceEntry,
};
use async_trait::async_trait;
use cloud_host::{build_router, set_test_run_overrides, AppState, Config, TestRunOverrides};
use serde_json::json;
use sqlx::PgPool;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tower::ServiceExt;
use uuid::Uuid;

struct MockComputer;

#[async_trait]
impl AgentComputer for MockComputer {
    async fn ensure_ready(&self) -> Result<ComputerInfo, ComputerError> {
        Ok(ComputerInfo {
            ready: true,
            protocol_version: 1,
            detail: Some("mock".into()),
        })
    }

    async fn list_dir(&self, _path: &str) -> Result<Vec<WorkspaceEntry>, ComputerError> {
        Ok(vec![])
    }

    async fn read_file(&self, path: &str) -> Result<Vec<u8>, ComputerError> {
        if path.contains("hello") {
            return Ok(b"hello from elsewhere".to_vec());
        }
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

struct SlowScriptedModel {
    delay: Duration,
    inner: ScriptedModel,
}

#[async_trait]
impl ResponsesModel for SlowScriptedModel {
    async fn create_response(
        &self,
        request: agent_core::CreateResponseRequest,
    ) -> Result<CreateResponseResult, ModelError> {
        tokio::time::sleep(self.delay).await;
        self.inner.create_response(request).await
    }
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

async fn try_test_pool() -> Option<PgPool> {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://elsewhere:elsewhere@127.0.0.1:5432/elsewhere".into());
    let pool = tokio::time::timeout(std::time::Duration::from_secs(2), PgPool::connect(&url))
        .await
        .ok()?
        .ok()?;
    sqlx::migrate!("./migrations").run(&pool).await.ok()?;
    Some(pool)
}

fn test_config() -> Config {
    Config {
        database_url: std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://elsewhere:elsewhere@127.0.0.1:5432/elsewhere".into()),
        openai_api_key: Some("test-key".into()),
        sprite_token: "test-sprite".into(),
        api_token: "test-token".into(),
        auth_mode: cloud_host::config::AuthMode::InternalToken,
        jwt_issuer: None,
        jwt_audience: None,
        jwt_jwks_url: None,
        cors_web_origin: None,
        allow_codex_login: false,
        sprites_api_base: "http://127.0.0.1:9".into(),
        max_concurrent_runs: 2,
        run_timeout_secs: 120,
        bind_addr: "127.0.0.1:0".into(),
        run_engine: cloud_host::run_engine_select::RunEngineMode::Responses,
        codex_executable: None,
    }
}

#[tokio::test]
async fn http_auth_and_idempotent_run() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping http_auth_and_idempotent_run: Postgres unavailable");
        return;
    };
    let config = test_config();
    let state = AppState::new(pool, config);
    let app = build_router(state.clone());

    let health = app
        .clone()
        .oneshot(
            http::Request::builder()
                .uri("/health")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health.status(), http::StatusCode::OK);

    let unauthorized = app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("POST")
                .uri("/v1/runs")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    r#"{"bot":{"id":"b1","name":"n","instructions":"i","computerId":"c1"},"message":"hi"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), http::StatusCode::UNAUTHORIZED);

    set_test_run_overrides(Some(TestRunOverrides {
        computer: Arc::new(MockComputer),
        model: Arc::new(ScriptedModel {
            steps: Mutex::new(vec![CreateResponseResult {
                output: vec![json!({
                    "type":"message",
                    "content":[{"type":"output_text","text":"hello from elsewhere"}]
                })],
                output_text: Some("hello from elsewhere".into()),
            }]),
        }),
    }));

    let body = r#"{
      "bot":{"id":"bot_demo","name":"Researcher","instructions":"test","computerId":"computer_demo"},
      "message":"Create hello"
    }"#;

    let create = app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("POST")
                .uri("/v1/runs")
                .header("authorization", "Bearer test-token")
                .header("Idempotency-Key", "idem-1")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), http::StatusCode::ACCEPTED);

    let duplicate = app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("POST")
                .uri("/v1/runs")
                .header("authorization", "Bearer test-token")
                .header("Idempotency-Key", "idem-1")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(duplicate.status(), http::StatusCode::ACCEPTED);

    set_test_run_overrides(None);
}

fn install_fast_mock() {
    set_test_run_overrides(Some(TestRunOverrides {
        computer: Arc::new(MockComputer),
        model: Arc::new(ScriptedModel {
            steps: Mutex::new(vec![CreateResponseResult {
                output: vec![json!({
                    "type":"message",
                    "content":[{"type":"output_text","text":"hello from elsewhere"}]
                })],
                output_text: Some("hello from elsewhere".into()),
            }]),
        }),
    }));
}

async fn post_run(
    app: &axum::Router,
    idempotency_key: &str,
    bot_id: &str,
    computer_id: &str,
    message: &str,
    conversation_id: Option<&str>,
) -> http::Response<axum::body::Body> {
    let conversation = conversation_id
        .map(|id| format!(",\"conversationId\":\"{id}\""))
        .unwrap_or_default();
    let body = format!(
        r#"{{"bot":{{"id":"{bot_id}","name":"n","instructions":"i","computerId":"{computer_id}"}}{conversation},"message":"{message}"}}"#,
    );
    app.clone()
        .oneshot(
            http::Request::builder()
                .method("POST")
                .uri("/v1/runs")
                .header("authorization", "Bearer test-token")
                .header("Idempotency-Key", idempotency_key)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn concurrency_cap_returns_429_when_saturated() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping concurrency_cap_returns_429_when_saturated");
        return;
    };
    let config = test_config();
    let state = AppState::new(pool, config);
    let app = build_router(state);

    set_test_run_overrides(Some(TestRunOverrides {
        computer: Arc::new(MockComputer),
        model: Arc::new(SlowScriptedModel {
            delay: Duration::from_secs(3),
            inner: ScriptedModel {
                steps: Mutex::new(vec![CreateResponseResult {
                    output: vec![json!({
                        "type":"message",
                        "content":[{"type":"output_text","text":"slow"}]
                    })],
                    output_text: Some("slow".into()),
                }]),
            },
        }),
    }));

    let suffix = Uuid::new_v4();
    let r1 = post_run(&app, &format!("c1-{suffix}"), "bot_c1", "comp_c1", "one", None).await;
    let r2 = post_run(&app, &format!("c2-{suffix}"), "bot_c2", "comp_c2", "two", None).await;
    assert_eq!(r1.status(), http::StatusCode::ACCEPTED);
    assert_eq!(r2.status(), http::StatusCode::ACCEPTED);

    let r3 = post_run(&app, &format!("c3-{suffix}"), "bot_c3", "comp_c3", "three", None).await;
    assert_eq!(r3.status(), http::StatusCode::TOO_MANY_REQUESTS);

    set_test_run_overrides(None);
}

#[tokio::test]
async fn concurrent_idempotency_creates_single_run() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping concurrent_idempotency_creates_single_run");
        return;
    };
    let config = test_config();
    let state = AppState::new(pool.clone(), config);
    let app = build_router(state);
    install_fast_mock();

    let key = format!("idem-concurrent-{}", Uuid::new_v4());
    let bot = format!("bot_idem_{}", Uuid::new_v4());
    let (a, b) = tokio::join!(
        post_run(&app, &key, &bot, "comp_idem", "hello", None),
        post_run(&app, &key, &bot, "comp_idem", "hello", None),
    );
    assert_eq!(a.status(), http::StatusCode::ACCEPTED);
    assert_eq!(b.status(), http::StatusCode::ACCEPTED);

    let body_a = axum::body::to_bytes(a.into_body(), usize::MAX).await.unwrap();
    let body_b = axum::body::to_bytes(b.into_body(), usize::MAX).await.unwrap();
    let json_a: serde_json::Value = serde_json::from_slice(&body_a).unwrap();
    let json_b: serde_json::Value = serde_json::from_slice(&body_b).unwrap();
    assert_eq!(json_a["runId"], json_b["runId"]);

    tokio::time::sleep(Duration::from_secs(2)).await;

    let run_id = json_a["runId"].as_str().unwrap();
    let messages: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM messages WHERE kind = 'chat' AND conversation_id = (SELECT conversation_id FROM agent_runs WHERE id = $1)",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(messages.0, 2);

    let runs: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM agent_runs WHERE request_id = $1")
            .bind(&key)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(runs.0, 1);

    set_test_run_overrides(None);
}

#[tokio::test]
async fn conversation_ownership_is_enforced() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping conversation_ownership_is_enforced");
        return;
    };
    let config = test_config();
    let app = build_router(AppState::new(pool.clone(), config));

    let bot_a = format!("bot_a_{}", Uuid::new_v4());
    let bot_b = format!("bot_b_{}", Uuid::new_v4());
    let conv = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO bots (id, owner_id, name, system_prompt, model) VALUES ($1, 'legacy-local', 'a', '', 'gpt-5.6-luna')")
        .bind(&bot_a)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO conversations (id, owner_id, bot_id) VALUES ($1, 'legacy-local', $2)")
        .bind(&conv)
        .bind(&bot_a)
        .execute(&pool)
        .await
        .unwrap();

    let resp = post_run(
        &app,
        &format!("own-{}", Uuid::new_v4()),
        &bot_b,
        "comp_own",
        "hi",
        Some(&conv),
    )
    .await;
    assert_eq!(resp.status(), http::StatusCode::CONFLICT);
}

#[tokio::test]
async fn message_sequences_increment_per_conversation() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping message_sequences_increment_per_conversation");
        return;
    };
    let config = test_config();
    let app = build_router(AppState::new(pool.clone(), config));
    install_fast_mock();

    let bot = format!("bot_seq_{}", Uuid::new_v4());
    let conv = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO bots (id, owner_id, name, system_prompt, model) VALUES ($1, 'legacy-local', 'a', '', 'gpt-5.6-luna')")
        .bind(&bot)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO conversations (id, owner_id, bot_id) VALUES ($1, 'legacy-local', $2)")
        .bind(&conv)
        .bind(&bot)
        .execute(&pool)
        .await
        .unwrap();

    let r1 = post_run(
        &app,
        &format!("seq1-{}", Uuid::new_v4()),
        &bot,
        "comp_seq",
        "first",
        Some(&conv),
    )
    .await;
    assert_eq!(r1.status(), http::StatusCode::ACCEPTED);
    tokio::time::sleep(Duration::from_secs(2)).await;

    let r2 = post_run(
        &app,
        &format!("seq2-{}", Uuid::new_v4()),
        &bot,
        "comp_seq",
        "second",
        Some(&conv),
    )
    .await;
    assert_eq!(r2.status(), http::StatusCode::ACCEPTED);
    tokio::time::sleep(Duration::from_secs(2)).await;

    let chat_sequences: Vec<(i64,)> = sqlx::query_as(
        "SELECT sequence FROM messages WHERE conversation_id = $1 AND kind = 'chat' ORDER BY sequence ASC",
    )
    .bind(&conv)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(chat_sequences.len(), 4);

    let uniqueness: (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COUNT(DISTINCT sequence) FROM messages WHERE conversation_id = $1",
    )
    .bind(&conv)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(uniqueness.0, uniqueness.1);

    let roles: Vec<(String, i64)> = sqlx::query_as(
        "SELECT role, sequence FROM messages WHERE conversation_id = $1 AND kind = 'chat' ORDER BY sequence ASC",
    )
    .bind(&conv)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(roles[0].0, "user");
    assert_eq!(roles[1].0, "assistant");
    assert_eq!(roles[2].0, "user");
    assert_eq!(roles[3].0, "assistant");
    assert!(roles[2].1 > roles[1].1);
    assert!(roles[3].1 > roles[2].1);

    set_test_run_overrides(None);
}

#[tokio::test]
async fn sse_reconnect_uses_monotonic_durable_ids() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping sse_reconnect_uses_monotonic_durable_ids");
        return;
    };
    let config = test_config();
    let app = build_router(AppState::new(pool.clone(), config));
    install_fast_mock();

    let key = format!("sse-{}", Uuid::new_v4());
    let bot = format!("bot_sse_{}", Uuid::new_v4());
    let create = post_run(&app, &key, &bot, "comp_sse", "hello", None).await;
    assert_eq!(create.status(), http::StatusCode::ACCEPTED);
    let body = axum::body::to_bytes(create.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let run_id = json["runId"].as_str().unwrap();

    tokio::time::sleep(Duration::from_secs(3)).await;

    let events: Vec<(i64,)> = sqlx::query_as(
        "SELECT id FROM run_events WHERE request_id = $1 ORDER BY id ASC",
    )
    .bind(&key)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(events.len() >= 2);

    let last_id = events[events.len() / 2].0;
    let sse = app
        .clone()
        .oneshot(
            http::Request::builder()
                .uri(format!("/v1/runs/{run_id}/events"))
                .header("authorization", "Bearer test-token")
                .header("Last-Event-ID", last_id.to_string())
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(sse.status(), http::StatusCode::OK);

    set_test_run_overrides(None);
}
