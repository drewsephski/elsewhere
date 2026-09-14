use agent_core::{
    AgentComputer, ComputerError, ComputerInfo, CreateResponseResult, ExecResult, ModelError,
    ResponsesModel, WorkspaceEntry,
};
use async_trait::async_trait;
use cloud_host::{build_router, set_test_run_overrides, AppState, Config, TestRunOverrides};
use serde_json::json;
use sqlx::PgPool;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

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
        openai_api_key: "test-key".into(),
        sprite_token: "test-sprite".into(),
        api_token: "test-token".into(),
        sprites_api_base: "http://127.0.0.1:9".into(),
        max_concurrent_runs: 2,
        run_timeout_secs: 120,
        bind_addr: "127.0.0.1:0".into(),
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
