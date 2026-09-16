//! Persistent Bot permission policies: precedence, isolation, and approval bypass.

use agent_core::{
    dispatch_tool_with_gate, AgentComputer, ComputerError, ComputerInfo, CreateResponseResult,
    ExecResult, ModelError, ResponsesModel, ToolError, ToolRunContext, WorkspaceEntry,
};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use cloud_host::approval::{BrowserHumanControlGate, RunScopedApprovalGate};
use cloud_host::auth::{JwtVerifier, JwtVerifierConfig};
use cloud_host::computer_control;
use cloud_host::config::{AuthMode, Config};
use cloud_host::db::resources::{insert_bot, insert_computer_placeholder};
use cloud_host::db::PostgresRunStore;
use cloud_host::events::cloud_event_sink::CloudEventSink;
use cloud_host::permission_policies::PolicyDecision;
use cloud_host::routines::{self, RoutineInput};
use cloud_host::{build_router, test_signing, AppState, TestRunOverrides};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration as StdDuration;
use tower::ServiceExt;
use uuid::Uuid;

const TEST_JWT_ISSUER: &str = "http://localhost:3000";
const TEST_JWT_AUDIENCE: &str = "elsewhere-cloud-host";

struct RunOverrideGuard {
    state: AppState,
    request_id: Option<String>,
}

impl RunOverrideGuard {
    fn install(
        state: AppState,
        request_id: String,
        computer: Arc<CountingComputer>,
        model: Arc<dyn ResponsesModel>,
    ) -> Self {
        state.register_test_run_overrides(&request_id, TestRunOverrides { computer, model });
        Self {
            state,
            request_id: Some(request_id),
        }
    }

    fn install_default(
        state: AppState,
        computer: Arc<CountingComputer>,
        model: Arc<dyn ResponsesModel>,
    ) -> Self {
        state.set_test_run_overrides_default(Some(TestRunOverrides { computer, model }));
        Self {
            state,
            request_id: None,
        }
    }
}

impl Drop for RunOverrideGuard {
    fn drop(&mut self) {
        if let Some(request_id) = &self.request_id {
            self.state.clear_test_run_overrides(request_id);
        } else {
            self.state.set_test_run_overrides_default(None);
        }
    }
}

struct CountingComputer {
    writes: AtomicUsize,
    execs: AtomicUsize,
    browser_calls: AtomicUsize,
}

impl CountingComputer {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            writes: AtomicUsize::new(0),
            execs: AtomicUsize::new(0),
            browser_calls: AtomicUsize::new(0),
        })
    }
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

    async fn browser_invoke(&self, _action: &str, _args: &Value) -> Result<Value, ComputerError> {
        self.browser_calls.fetch_add(1, Ordering::SeqCst);
        Ok(json!({ "ok": true }))
    }
}

struct ScriptedModel {
    steps: Mutex<Vec<CreateResponseResult>>,
}

impl ScriptedModel {
    fn write_then_done() -> Arc<Self> {
        Self::calls(
            "workspace_write",
            r#"{"path":"/workspace/a.txt","content":"hi"}"#,
        )
    }

    fn subagent_then_done() -> Arc<Self> {
        Arc::new(Self {
            steps: Mutex::new(vec![
                CreateResponseResult {
                    output: vec![json!({
                        "type": "function_call",
                        "name": "run_subagent",
                        "call_id": "c1",
                        "arguments": r#"{"name":"Reviewer","task":"review the implementation"}"#
                    })],
                    output_text: None,
                },
                CreateResponseResult {
                    output: vec![json!({
                        "type": "message",
                        "content": [{"type":"output_text","text":"helper findings"}]
                    })],
                    output_text: Some("helper findings".into()),
                },
                CreateResponseResult {
                    output: vec![json!({
                        "type": "message",
                        "content": [{"type":"output_text","text":"parent continued"}]
                    })],
                    output_text: Some("parent continued".into()),
                },
            ]),
        })
    }

    fn calls(name: &str, arguments: &str) -> Arc<Self> {
        Arc::new(Self {
            steps: Mutex::new(vec![
                CreateResponseResult {
                    output: vec![json!({
                        "type": "function_call",
                        "name": name,
                        "call_id": "c1",
                        "arguments": arguments
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
        browser_enabled: true,
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

async fn json_body(response: axum::http::Response<axum::body::Body>) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(json!({}))
}

async fn auth_json(
    app: &axum::Router,
    method: &str,
    uri: &str,
    owner: &str,
    body: Option<Value>,
) -> (axum::http::StatusCode, Value) {
    let mut builder = axum::http::Request::builder()
        .method(method)
        .uri(uri)
        .header("Authorization", format!("Bearer {}", token(owner)));
    let request_body = if let Some(body) = body {
        builder = builder.header("content-type", "application/json");
        axum::body::Body::from(body.to_string())
    } else {
        axum::body::Body::empty()
    };
    let response = app
        .clone()
        .oneshot(builder.body(request_body).unwrap())
        .await
        .unwrap();
    (response.status(), json_body(response).await)
}

async fn seed_bot(pool: &PgPool, owner: &str) -> (String, String) {
    let computer = insert_computer_placeholder(pool, owner, "c").await.unwrap();
    let bot = insert_bot(
        pool,
        owner,
        "Scout",
        "i",
        "gpt-5.6-luna",
        Some(computer.id.as_str()),
        "responses",
        "sky-wisp",
    )
    .await
    .unwrap();
    (bot.id, computer.id)
}

async fn start_write_run(
    state: &AppState,
    owner: &str,
    bot_id: &str,
    computer: Arc<CountingComputer>,
    model: Arc<dyn ResponsesModel>,
) -> (axum::Router, RunOverrideGuard, String) {
    let request_id = Uuid::new_v4().to_string();
    let guard = RunOverrideGuard::install(state.clone(), request_id.clone(), computer, model);
    let app = build_router(state.clone());
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/runs")
                .header("Authorization", format!("Bearer {}", token(owner)))
                .header("Idempotency-Key", &request_id)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({ "botId": bot_id, "message": "write" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::ACCEPTED);
    cloud_host::worker::dispatch_available(state).await.unwrap();
    (app, guard, request_id)
}

async fn pending_count(pool: &PgPool, owner: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM tool_approval_requests WHERE owner_id = $1 AND status = 'pending'",
    )
    .bind(owner)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn wait_pending(pool: &PgPool, owner: &str) -> bool {
    for _ in 0..40 {
        tokio::time::sleep(StdDuration::from_millis(100)).await;
        if pending_count(pool, owner).await >= 1 {
            return true;
        }
    }
    false
}

async fn wait_writes(computer: &CountingComputer, expected: usize) -> bool {
    for _ in 0..40 {
        tokio::time::sleep(StdDuration::from_millis(100)).await;
        if computer.writes.load(Ordering::SeqCst) >= expected {
            return true;
        }
    }
    false
}

async fn wait_no_active_runs(pool: &PgPool, owner: &str) -> bool {
    for _ in 0..50 {
        tokio::time::sleep(StdDuration::from_millis(100)).await;
        let active: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM agent_runs WHERE owner_id = $1 AND status IN ('queued', 'running')",
        )
        .bind(owner)
        .fetch_one(pool)
        .await
        .unwrap();
        if active == 0 {
            return true;
        }
    }
    false
}

async fn policy_event_count(pool: &PgPool, request_id: &str, decision: &str) -> i64 {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM run_events
        WHERE request_id = $1
          AND event_type = 'permission_policy_resolved'
          AND payload_json->>'decision' = $2
        "#,
    )
    .bind(request_id)
    .bind(decision)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn default_catalog_asks_and_does_not_allow_broad_permissions(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let app = build_router(jwt_state(pool));
    let (status, body) = auth_json(
        &app,
        "GET",
        "/v1/settings/permission-policies",
        &owner,
        None,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let actions = body["actions"].as_array().expect("actions");
    assert!(actions.iter().all(|item| item["decision"] == "ask"));
    assert!(actions.iter().all(|item| item["inherited"] == true));
    assert!(actions
        .iter()
        .any(|item| item["action"] == "workspace_write"));
    assert!(!actions
        .iter()
        .any(|item| item["action"] == "browser_request_human"));
}

#[sqlx::test(migrations = "./migrations")]
async fn mutations_still_ask_without_stored_policy(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let state = jwt_state(pool.clone());
    let computer = CountingComputer::new();
    let (_, _guard, _) = start_write_run(
        &state,
        &owner,
        &bot_id,
        computer.clone(),
        ScriptedModel::write_then_done(),
    )
    .await;
    assert!(wait_pending(&pool, &owner).await);
    assert_eq!(computer.writes.load(Ordering::SeqCst), 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn allow_skips_approval_but_still_records_audit(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let state = jwt_state(pool.clone());
    let app = build_router(state.clone());
    let (status, _) = auth_json(
        &app,
        "PUT",
        &format!("/v1/bots/{bot_id}/permission-policies"),
        &owner,
        Some(json!({ "policies": [{ "action": "workspace_write", "decision": "allow" }] })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);

    let computer = CountingComputer::new();
    let request_id = Uuid::new_v4().to_string();
    let _guard = RunOverrideGuard::install(
        state.clone(),
        request_id.clone(),
        computer.clone(),
        ScriptedModel::write_then_done(),
    );
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/runs")
                .header("Authorization", format!("Bearer {}", token(&owner)))
                .header("Idempotency-Key", &request_id)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({ "botId": bot_id, "message": "write" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::ACCEPTED);
    cloud_host::worker::dispatch_available(&state)
        .await
        .unwrap();
    assert!(wait_writes(&computer, 1).await);
    assert_eq!(pending_count(&pool, &owner).await, 0);
    assert!(policy_event_count(&pool, &request_id, "allow").await >= 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn deny_rejects_without_approval_row(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let state = jwt_state(pool.clone());
    state
        .permission_policies
        .upsert_bot_decision(
            &owner,
            &bot_id,
            "workspace_write",
            Some(PolicyDecision::Deny),
        )
        .await
        .unwrap();
    let computer = CountingComputer::new();
    let request_id = Uuid::new_v4().to_string();
    let _guard = RunOverrideGuard::install(
        state.clone(),
        request_id.clone(),
        computer.clone(),
        ScriptedModel::write_then_done(),
    );
    let app = build_router(state.clone());
    let _ = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/runs")
                .header("Authorization", format!("Bearer {}", token(&owner)))
                .header("Idempotency-Key", &request_id)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({ "botId": bot_id, "message": "write" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    cloud_host::worker::dispatch_available(&state)
        .await
        .unwrap();
    for _ in 0..40 {
        tokio::time::sleep(StdDuration::from_millis(100)).await;
        if policy_event_count(&pool, &request_id, "deny").await >= 1 {
            break;
        }
    }
    assert_eq!(computer.writes.load(Ordering::SeqCst), 0);
    let approvals: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tool_approval_requests WHERE owner_id = $1")
            .bind(&owner)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(approvals, 0);
    assert!(policy_event_count(&pool, &request_id, "deny").await >= 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn bot_override_beats_owner_default_and_removing_restores_inheritance(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let state = jwt_state(pool.clone());
    let app = build_router(state.clone());
    let (status, _) = auth_json(
        &app,
        "PUT",
        "/v1/settings/permission-policies",
        &owner,
        Some(json!({ "policies": [{ "action": "workspace_write", "decision": "deny" }] })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let (status, body) = auth_json(
        &app,
        "PUT",
        &format!("/v1/bots/{bot_id}/permission-policies"),
        &owner,
        Some(json!({ "policies": [{ "action": "workspace_write", "decision": "allow" }] })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(body["actions"][0]["action"], "workspace_write");
    let write = body["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["action"] == "workspace_write")
        .unwrap();
    assert_eq!(write["decision"], "allow");
    assert_eq!(write["source"], "bot");
    assert_eq!(write["inherited"], false);

    let resolved = state
        .permission_policies
        .resolve(&owner, &bot_id, "workspace_write")
        .await
        .unwrap();
    assert_eq!(resolved.decision, PolicyDecision::Allow);
    assert_eq!(resolved.source.as_str(), "bot");

    let (status, body) = auth_json(
        &app,
        "PUT",
        &format!("/v1/bots/{bot_id}/permission-policies"),
        &owner,
        Some(json!({ "policies": [{ "action": "workspace_write", "decision": null }] })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let write = body["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["action"] == "workspace_write")
        .unwrap();
    assert_eq!(write["decision"], "deny");
    assert_eq!(write["inherited"], true);
    assert_eq!(write["source"], "owner");
}

#[sqlx::test(migrations = "./migrations")]
async fn foreign_bot_policies_are_rejected(pool: PgPool) {
    let owner_a = format!("owner-a-{}", Uuid::new_v4());
    let owner_b = format!("owner-b-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner_a).await;
    let app = build_router(jwt_state(pool));
    let (status, _) = auth_json(
        &app,
        "PUT",
        &format!("/v1/bots/{bot_id}/permission-policies"),
        &owner_b,
        Some(json!({ "policies": [{ "action": "workspace_write", "decision": "allow" }] })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
    let (status, _) = auth_json(
        &app,
        "GET",
        &format!("/v1/bots/{bot_id}/permission-policies"),
        &owner_b,
        None,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn non_overridable_and_unknown_actions_are_rejected(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let app = build_router(jwt_state(pool));
    for action in ["browser_request_human", "workspace_read", "not_a_tool"] {
        let (status, _) = auth_json(
            &app,
            "PUT",
            &format!("/v1/bots/{bot_id}/permission-policies"),
            &owner,
            Some(json!({ "policies": [{ "action": action, "decision": "allow" }] })),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::BAD_REQUEST, "{action}");
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn always_allow_from_approval_is_atomic(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let state = jwt_state(pool.clone());
    let computer = CountingComputer::new();
    let (app, _guard, _) = start_write_run(
        &state,
        &owner,
        &bot_id,
        computer.clone(),
        ScriptedModel::write_then_done(),
    )
    .await;
    assert!(wait_pending(&pool, &owner).await);
    let approval_id: String = sqlx::query_scalar(
        "SELECT id FROM tool_approval_requests WHERE owner_id = $1 AND status = 'pending'",
    )
    .bind(&owner)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (status, body) = auth_json(
        &app,
        "POST",
        &format!("/v1/approvals/{approval_id}/always-allow"),
        &owner,
        None,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(body["policy"]["decision"], "allow");
    assert_eq!(body["policy"]["action"], "workspace_write");
    assert_eq!(body["policy"]["botId"], bot_id);
    assert!(wait_writes(&computer, 1).await);
    let stored: String = sqlx::query_scalar(
        "SELECT decision FROM bot_permission_policies WHERE bot_id = $1 AND action_key = 'workspace_write'",
    )
    .bind(&bot_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored, "allow");
    let pending_after = pending_count(&pool, &owner).await;
    assert_eq!(pending_after, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn allow_does_not_bypass_url_or_path_validators(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, computer_id) = seed_bot(&pool, &owner).await;
    let state = jwt_state(pool.clone());
    state
        .permission_policies
        .upsert_bot_decision(
            &owner,
            &bot_id,
            "browser_navigate",
            Some(PolicyDecision::Allow),
        )
        .await
        .unwrap();
    state
        .permission_policies
        .upsert_bot_decision(
            &owner,
            &bot_id,
            "browser_screenshot",
            Some(PolicyDecision::Allow),
        )
        .await
        .unwrap();

    let computer = CountingComputer::new();
    let cancel = AtomicBool::new(false);
    let run = ToolRunContext {
        run_id: "run-1".into(),
        request_id: "req-1".into(),
        owner_id: owner.clone(),
        bot_id: bot_id.clone(),
        computer_id,
        tool_invocation_id: None,
    };
    let (events, _rx) = CloudEventSink::new();
    let store = Arc::new(PostgresRunStore::new(pool.clone()));
    let gate = RunScopedApprovalGate::new(
        state.approvals.clone(),
        state.permission_policies.clone(),
        Arc::new(events),
        store,
        Arc::new(AtomicBool::new(false)),
    );

    let blocked = dispatch_tool_with_gate(
        computer.as_ref(),
        "browser_navigate",
        r#"{"url":"http://127.0.0.1/"}"#,
        &cancel,
        &gate,
        &run,
    )
    .await;
    assert!(matches!(blocked, Err(ToolError::MalformedArguments(_))));
    assert_eq!(computer.browser_calls.load(Ordering::SeqCst), 0);
    assert_eq!(pending_count(&pool, &owner).await, 0);

    let screenshot = dispatch_tool_with_gate(
        computer.as_ref(),
        "browser_screenshot",
        r#"{"path":"/etc/passwd"}"#,
        &cancel,
        &gate,
        &run,
    )
    .await;
    assert!(matches!(screenshot, Err(ToolError::MalformedArguments(_))));
    assert_eq!(computer.browser_calls.load(Ordering::SeqCst), 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn allow_does_not_bypass_human_control_or_cancellation(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, computer_id) = seed_bot(&pool, &owner).await;
    let state = jwt_state(pool.clone());
    state
        .permission_policies
        .upsert_bot_decision(
            &owner,
            &bot_id,
            "browser_click",
            Some(PolicyDecision::Allow),
        )
        .await
        .unwrap();
    computer_control::take_human_control(&pool, &owner, &computer_id)
        .await
        .unwrap();

    let cancel = Arc::new(AtomicBool::new(false));
    let (events, _rx) = CloudEventSink::new();
    let store = Arc::new(PostgresRunStore::new(pool.clone()));
    let gate = BrowserHumanControlGate::wrapping_run_gate(
        RunScopedApprovalGate::new(
            state.approvals.clone(),
            state.permission_policies.clone(),
            Arc::new(events),
            store,
            cancel.clone(),
        ),
        pool.clone(),
    );
    let computer = CountingComputer::new();
    let run = ToolRunContext {
        run_id: "run-1".into(),
        request_id: "req-1".into(),
        owner_id: owner.clone(),
        bot_id,
        computer_id: computer_id.clone(),
        tool_invocation_id: None,
    };

    let owner_for_release = owner.clone();
    let pool_for_release = pool.clone();
    let computer_id_for_release = computer_id.clone();
    let release = tokio::spawn(async move {
        tokio::time::sleep(StdDuration::from_millis(400)).await;
        computer_control::return_control_to_bot(
            &pool_for_release,
            &owner_for_release,
            &computer_id_for_release,
        )
        .await
        .unwrap();
    });
    let result = dispatch_tool_with_gate(
        computer.as_ref(),
        "browser_click",
        r#"{"ref":"e1"}"#,
        &cancel,
        &gate,
        &run,
    )
    .await
    .expect("click after human release");
    release.await.unwrap();
    assert_eq!(result.get("ok"), Some(&json!(true)));
    assert_eq!(computer.browser_calls.load(Ordering::SeqCst), 1);

    cancel.store(true, Ordering::Relaxed);
    let cancelled = dispatch_tool_with_gate(
        computer.as_ref(),
        "browser_click",
        r#"{"ref":"e1"}"#,
        &cancel,
        &gate,
        &run,
    )
    .await;
    assert!(matches!(cancelled, Err(ToolError::Cancelled)));
}

#[sqlx::test(migrations = "./migrations")]
async fn unattended_routine_and_webhook_use_the_same_policy_path(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let state = jwt_state(pool.clone());
    state
        .permission_policies
        .upsert_bot_decision(
            &owner,
            &bot_id,
            "workspace_write",
            Some(PolicyDecision::Allow),
        )
        .await
        .unwrap();

    let scheduled = routines::save(
        &pool,
        &owner,
        None,
        &RoutineInput {
            bot_id: bot_id.clone(),
            name: "Nightly".into(),
            instructions: "Write notes".into(),
            interval_minutes: Some(60),
            next_run_at: Utc::now() - Duration::minutes(1),
            enabled: true,
            schedule_kind: None,
            schedule_expression: None,
            timezone: None,
            destination_conversation_id: None,
            failure_policy: None,
            skill_id: None,
            pinned_skill_version: None,
            trigger_mode: None,
        },
    )
    .await
    .unwrap();
    sqlx::query("UPDATE routines SET next_run_at = NOW() - interval '1 second' WHERE id = $1")
        .bind(&scheduled.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(routines::tick(&pool, Utc::now()).await.unwrap(), 1);

    let computer = CountingComputer::new();
    {
        let _guard = RunOverrideGuard::install_default(
            state.clone(),
            computer.clone(),
            ScriptedModel::write_then_done(),
        );
        cloud_host::worker::dispatch_available(&state)
            .await
            .unwrap();
        assert!(wait_writes(&computer, 1).await);
        assert_eq!(pending_count(&pool, &owner).await, 0);
        assert!(wait_no_active_runs(&pool, &owner).await);
    }

    let webhook = routines::save(
        &pool,
        &owner,
        None,
        &RoutineInput {
            bot_id: bot_id.clone(),
            name: "Hook".into(),
            instructions: "Write notes".into(),
            interval_minutes: Some(60),
            next_run_at: Utc::now() + Duration::hours(1),
            enabled: true,
            schedule_kind: None,
            schedule_expression: None,
            timezone: None,
            destination_conversation_id: None,
            failure_policy: None,
            skill_id: None,
            pinned_skill_version: None,
            trigger_mode: Some("webhook".into()),
        },
    )
    .await
    .unwrap();
    let token = webhook
        .webhook
        .webhook_url
        .as_deref()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap()
        .to_string();
    routines::admit_webhook_event(&pool, &token, json!({}), None, None)
        .await
        .unwrap();
    let _guard = RunOverrideGuard::install_default(
        state.clone(),
        computer.clone(),
        ScriptedModel::write_then_done(),
    );
    cloud_host::worker::dispatch_available(&state)
        .await
        .unwrap();
    assert!(wait_writes(&computer, 2).await);
    assert_eq!(pending_count(&pool, &owner).await, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn allow_run_subagent_returns_to_parent_without_new_bot_or_run(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let state = jwt_state(pool.clone());
    state
        .permission_policies
        .upsert_bot_decision(&owner, &bot_id, "run_subagent", Some(PolicyDecision::Allow))
        .await
        .unwrap();
    let computer = CountingComputer::new();
    let request_id = Uuid::new_v4().to_string();
    let _guard = RunOverrideGuard::install(
        state.clone(),
        request_id.clone(),
        computer.clone(),
        ScriptedModel::subagent_then_done(),
    );
    let app = build_router(state.clone());
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/runs")
                .header("Authorization", format!("Bearer {}", token(&owner)))
                .header("Idempotency-Key", &request_id)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({ "botId": bot_id, "message": "review" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::ACCEPTED);
    cloud_host::worker::dispatch_available(&state)
        .await
        .unwrap();
    assert!(wait_no_active_runs(&pool, &owner).await);
    let helpers: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM run_subagents WHERE tool_invocation_id = 'c1'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(helpers, 1);
    let status: String =
        sqlx::query_scalar("SELECT status FROM run_subagents WHERE tool_invocation_id = 'c1'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "completed");
    let extra_bots: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bots WHERE owner_id = $1")
        .bind(&owner)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(extra_bots, 1);
    let extra_runs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agent_runs ar JOIN bots b ON b.id = ar.bot_id WHERE b.owner_id = $1",
    )
    .bind(&owner)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(extra_runs, 1);
    let helper_result: Option<String> =
        sqlx::query_scalar("SELECT result FROM run_subagents WHERE tool_invocation_id = 'c1'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(helper_result.as_deref(), Some("helper findings"));
    let run_status: String =
        sqlx::query_scalar("SELECT status FROM agent_runs WHERE request_id = $1")
            .bind(&request_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(run_status, "completed");
    let assistant: String = sqlx::query_scalar(
        "SELECT m.body FROM agent_runs ar JOIN messages m ON m.id = ar.assistant_message_id WHERE ar.request_id = $1",
    )
    .bind(&request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        assistant.contains("parent continued") || assistant.contains("helper findings"),
        "{assistant}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn deny_run_subagent_does_not_launch_helper(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let state = jwt_state(pool.clone());
    state
        .permission_policies
        .upsert_bot_decision(&owner, &bot_id, "run_subagent", Some(PolicyDecision::Deny))
        .await
        .unwrap();
    let computer = CountingComputer::new();
    let request_id = Uuid::new_v4().to_string();
    let _guard = RunOverrideGuard::install(
        state.clone(),
        request_id.clone(),
        computer,
        ScriptedModel::calls(
            "run_subagent",
            r#"{"name":"Reviewer","task":"review the implementation"}"#,
        ),
    );
    let app = build_router(state.clone());
    let _ = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/runs")
                .header("Authorization", format!("Bearer {}", token(&owner)))
                .header("Idempotency-Key", &request_id)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({ "botId": bot_id, "message": "review" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    cloud_host::worker::dispatch_available(&state)
        .await
        .unwrap();
    assert!(wait_no_active_runs(&pool, &owner).await);
    let helpers: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM run_subagents")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(helpers, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn ask_run_subagent_creates_approval_and_does_not_launch(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let state = jwt_state(pool.clone());
    let computer = CountingComputer::new();
    let request_id = Uuid::new_v4().to_string();
    let _guard = RunOverrideGuard::install(
        state.clone(),
        request_id.clone(),
        computer,
        ScriptedModel::calls(
            "run_subagent",
            r#"{"name":"Reviewer","task":"review the implementation"}"#,
        ),
    );
    let app = build_router(state.clone());
    let _ = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/runs")
                .header("Authorization", format!("Bearer {}", token(&owner)))
                .header("Idempotency-Key", &request_id)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({ "botId": bot_id, "message": "review" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    cloud_host::worker::dispatch_available(&state)
        .await
        .unwrap();
    assert!(wait_pending(&pool, &owner).await);
    let helpers: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM run_subagents")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(helpers, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn connected_app_mutation_uses_exact_scoped_policy_not_global_allow(pool: PgPool) {
    use cloud_host::permission_policies::{PermissionPolicyService, PolicyDecision, PolicySource};

    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    sqlx::query(
        r#"
        INSERT INTO bot_permission_policies (
            id, owner_id, bot_id, action_key, decision, resource_scope, scope_key, created_at, updated_at
        )
        VALUES
            ($1, $2, $3, 'connected_apps_execute_tool', 'allow', '{}'::jsonb, '', NOW(), NOW()),
            ($4, $2, $3, 'connected_apps_execute_tool', 'allow', '{"installId":"inst-a","remoteTool":"create_doc"}'::jsonb,
             'install:inst-a/tool:create_doc', NOW(), NOW()),
            ($5, $2, $3, 'connected_apps_execute_tool', 'deny', '{"installId":"inst-a","remoteTool":"delete_doc"}'::jsonb,
             'install:inst-a/tool:delete_doc', NOW(), NOW())
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&owner)
    .bind(&bot_id)
    .bind(Uuid::new_v4().to_string())
    .bind(Uuid::new_v4().to_string())
    .execute(&pool)
    .await
    .unwrap();

    let policies = PermissionPolicyService::new(pool);
    let unscoped = policies
        .resolve(&owner, &bot_id, "connected_apps_execute_tool")
        .await
        .unwrap();
    assert_eq!(unscoped.decision, PolicyDecision::Ask);
    assert!(!unscoped.overridable);

    let allowed = policies
        .resolve_scoped(
            &owner,
            &bot_id,
            "connected_apps_execute_tool",
            "install:inst-a/tool:create_doc",
        )
        .await
        .unwrap();
    assert_eq!(allowed.decision, PolicyDecision::Allow);
    assert_eq!(allowed.source, PolicySource::Bot);

    let denied = policies
        .resolve_scoped(
            &owner,
            &bot_id,
            "connected_apps_execute_tool",
            "install:inst-a/tool:delete_doc",
        )
        .await
        .unwrap();
    assert_eq!(denied.decision, PolicyDecision::Deny);

    let other_tool = policies
        .resolve_scoped(
            &owner,
            &bot_id,
            "connected_apps_execute_tool",
            "install:inst-a/tool:other",
        )
        .await
        .unwrap();
    assert_eq!(other_tool.decision, PolicyDecision::Ask);

    assert!(
        PermissionPolicyService::validate_overridable_action("connected_apps_execute_tool")
            .is_err()
    );
}
