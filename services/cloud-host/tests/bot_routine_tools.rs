//! Bot-facing routine tools: schedules, approval, and owner isolation.

use agent_core::{CreateResponseResult, ModelError, ResponsesModel};
use async_trait::async_trait;
use chrono::Utc;
use cloud_host::auth::{JwtVerifier, JwtVerifierConfig};
use cloud_host::config::{AuthMode, Config};
use cloud_host::db::resources::{insert_bot, insert_computer_placeholder};
use cloud_host::routines::{self, RoutineInput};
use cloud_host::{build_router, test_signing, AppState, TestRunOverrides};
use http_body_util::BodyExt;
use serde_json::json;
use sqlx::PgPool;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use uuid::Uuid;

const TEST_JWT_ISSUER: &str = "http://localhost:3000";
const TEST_JWT_AUDIENCE: &str = "elsewhere-cloud-host";

struct ScriptedModel {
    steps: Mutex<Vec<CreateResponseResult>>,
}

impl ScriptedModel {
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
        github_app_slug: None,
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

async fn seed_bot(pool: &PgPool, owner: &str) -> (String, String) {
    let computer = insert_computer_placeholder(pool, owner, "Computer")
        .await
        .unwrap();
    let bot = insert_bot(
        pool,
        owner,
        "Scout",
        "Helpful",
        "gpt-5.6-luna",
        Some(&computer.id),
        "codex",
        "sky-wisp",
    )
    .await
    .unwrap();
    (bot.id, computer.id)
}

async fn wait_no_active_runs(pool: &PgPool, owner: &str) {
    for _ in 0..120 {
        let running: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM agent_runs WHERE owner_id = $1 AND status IN ('queued','running')",
        )
        .bind(owner)
        .fetch_one(pool)
        .await
        .unwrap();
        if running == 0 {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("runs did not finish");
}

async fn pending_approval_count(pool: &PgPool, owner: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM tool_approval_requests WHERE owner_id = $1 AND status = 'pending'",
    )
    .bind(owner)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn wait_pending_approval(pool: &PgPool, owner: &str) -> String {
    for _ in 0..80 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
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

async fn start_run(
    state: &AppState,
    owner: &str,
    bot_id: &str,
    message: &str,
    request_id: &str,
    model: Arc<dyn ResponsesModel>,
) {
    state.register_test_run_overrides(
        request_id,
        TestRunOverrides {
            computer: Some(Arc::new(agent_core::FakeAgentComputer::new())),
            model,
        },
    );
    let app = build_router(state.clone());
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/runs")
                .header("Authorization", format!("Bearer {}", token(owner)))
                .header("Idempotency-Key", request_id)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({ "botId": bot_id, "message": message }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::ACCEPTED);
    cloud_host::worker::dispatch_available(state).await.unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn routine_create_requires_approval_and_persists_timezone(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let state = jwt_state(pool.clone());
    let request_id = Uuid::new_v4().to_string();
    let args = json!({
        "name": "Morning brief",
        "instructions": "Summarize overnight competitor news.",
        "timezone": "America/Chicago",
        "schedule": { "repeat": "weekdays", "at": "08:00" }
    });
    start_run(
        &state,
        &owner,
        &bot_id,
        "create routine",
        &request_id,
        ScriptedModel::calls("routine_create", &args.to_string()),
    )
    .await;
    let approval_id = wait_pending_approval(&pool, &owner).await;
    let app = build_router(state.clone());
    let approve = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/v1/approvals/{approval_id}/approve"))
                .header("Authorization", format!("Bearer {}", token(&owner)))
                .body(axum::body::Body::new())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(approve.status(), axum::http::StatusCode::OK);
    cloud_host::worker::dispatch_available(&state).await.unwrap();
    wait_no_active_runs(&pool, &owner).await;
    state.clear_test_run_overrides(&request_id);

    let rows = routines::list(&pool, &owner).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].timezone, "America/Chicago");
    assert_eq!(rows[0].schedule_kind, "weekly");
    assert!(rows[0].schedule_expression.contains("weekdays|08:00"));
}

#[sqlx::test(migrations = "./migrations")]
async fn denied_routine_create_does_not_persist(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let state = jwt_state(pool.clone());
    let request_id = Uuid::new_v4().to_string();
    let args = json!({
        "name": "Denied",
        "instructions": "Should not exist",
        "timezone": "UTC",
        "schedule": { "repeat": "daily", "at": "09:00" }
    });
    start_run(
        &state,
        &owner,
        &bot_id,
        "create",
        &request_id,
        ScriptedModel::calls("routine_create", &args.to_string()),
    )
    .await;
    let approval_id = wait_pending_approval(&pool, &owner).await;
    let app = build_router(state.clone());
    let deny = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/v1/approvals/{approval_id}/deny"))
                .header("Authorization", format!("Bearer {}", token(&owner)))
                .body(axum::body::Body::new())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deny.status(), axum::http::StatusCode::OK);
    cloud_host::worker::dispatch_available(&state).await.unwrap();
    wait_no_active_runs(&pool, &owner).await;
    state.clear_test_run_overrides(&request_id);
    assert!(routines::list(&pool, &owner).await.unwrap().is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn routine_list_is_scoped_to_bot(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_a, _) = seed_bot(&pool, &owner).await;
    let (bot_b, _) = seed_bot(&pool, &owner).await;
    let now = Utc::now();
    for (bot_id, name) in [(bot_a.as_str(), "A"), (bot_b.as_str(), "B")] {
        let input = RoutineInput {
            bot_id: bot_id.clone(),
            name: format!("Routine {name}"),
            instructions: "work".into(),
            interval_minutes: Some(60),
            next_run_at: now,
            enabled: true,
            schedule_kind: Some("daily".into()),
            schedule_expression: Some("08:00".into()),
            timezone: Some("UTC".into()),
            destination_conversation_id: None,
            failure_policy: None,
            skill_id: None,
            pinned_skill_version: None,
            trigger_mode: Some("schedule".into()),
        };
        routines::save(&pool, &owner, None, &input).await.unwrap();
    }
    let service = cloud_host::agent_routines::PostgresAgentRoutines::new(pool.clone());
    let listed = service
        .list(&agent_core::RoutineContext {
            owner_id: owner.clone(),
            bot_id: bot_a.clone(),
            source_conversation_id: "conv".into(),
        })
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "Routine A");
}

#[sqlx::test(migrations = "./migrations")]
async fn invalid_routine_create_does_not_request_approval(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let state = jwt_state(pool.clone());
    let request_id = Uuid::new_v4().to_string();
    let args = json!({
        "name": "Morning brief",
        "instructions": "Summarize news.",
        "timezone": "America/Chicago",
        "schedule": { "repeat": "weekdays" }
    });
    start_run(
        &state,
        &owner,
        &bot_id,
        "create routine",
        &request_id,
        ScriptedModel::calls("routine_create", &args.to_string()),
    )
    .await;
    wait_no_active_runs(&pool, &owner).await;
    state.clear_test_run_overrides(&request_id);
    assert_eq!(pending_approval_count(&pool, &owner).await, 0);
    assert!(routines::list(&pool, &owner).await.unwrap().is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn validate_create_rejects_schedule_and_timezone_errors(pool: PgPool) {
    use agent_core::{BotRoutineSchedule, RoutineContext};
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let service = cloud_host::agent_routines::PostgresAgentRoutines::new(pool.clone());
    let ctx = RoutineContext {
        owner_id: owner.clone(),
        bot_id: bot_id.clone(),
        source_conversation_id: "conv-src".into(),
    };

    let cases: Vec<(BotRoutineSchedule, &str, &str)> = vec![
        (
            BotRoutineSchedule {
                repeat: "daily".into(),
                every_minutes: None,
                at: None,
                days: None,
            },
            "UTC",
            "Daily schedules need at",
        ),
        (
            BotRoutineSchedule {
                repeat: "daily".into(),
                every_minutes: None,
                at: Some("99:99".into()),
                days: None,
            },
            "UTC",
            "hh:mm",
        ),
        (
            BotRoutineSchedule {
                repeat: "daily".into(),
                every_minutes: None,
                at: Some("08:00".into()),
                days: None,
            },
            "Not/A/Timezone",
            "iana timezone",
        ),
        (
            BotRoutineSchedule {
                repeat: "weekly".into(),
                every_minutes: None,
                at: Some("08:00".into()),
                days: Some("NOTADAY".into()),
            },
            "UTC",
            "weekly days",
        ),
        (
            BotRoutineSchedule {
                repeat: "every_minutes".into(),
                every_minutes: Some(5),
                at: None,
                days: None,
            },
            "UTC",
            "15 minutes",
        ),
    ];

    for (schedule, timezone, needle) in cases {
        let err = service
            .validate_create(&ctx, "Test", "Do work", &schedule, timezone, None)
            .await
            .unwrap_err();
        let message = err.message().to_lowercase();
        assert!(
            message.contains(&needle.to_lowercase()),
            "expected {needle} in {message}"
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn malformed_schedule_is_rejected(pool: PgPool) {
    let spec = agent_core::BotRoutineSchedule {
        repeat: "cron".into(),
        every_minutes: None,
        at: None,
        days: None,
    };
    assert!(cloud_host::agent_routines::schedule_to_routine_fields(&spec, "UTC").is_err());
}

#[sqlx::test(migrations = "./migrations")]
async fn pause_other_owners_routine_is_forbidden(pool: PgPool) {
    let alice = format!("alice-{}", Uuid::new_v4());
    let bob = format!("bob-{}", Uuid::new_v4());
    let (alice_bot, _) = seed_bot(&pool, &alice).await;
    let (bob_bot, _) = seed_bot(&pool, &bob).await;
    let view = routines::save(
        &pool,
        &alice,
        None,
        &RoutineInput {
            bot_id: alice_bot.clone(),
            name: "Alice only".into(),
            instructions: "secret".into(),
            interval_minutes: Some(60),
            next_run_at: Utc::now(),
            enabled: true,
            schedule_kind: Some("daily".into()),
            schedule_expression: Some("08:00".into()),
            timezone: Some("UTC".into()),
            destination_conversation_id: None,
            failure_policy: None,
            skill_id: None,
            pinned_skill_version: None,
            trigger_mode: Some("schedule".into()),
        },
    )
    .await
    .unwrap();
    let service = cloud_host::agent_routines::PostgresAgentRoutines::new(pool.clone());
    let err = service
        .set_enabled(
            &agent_core::RoutineContext {
                owner_id: bob.clone(),
                bot_id: bob_bot,
                source_conversation_id: "conv".into(),
            },
            &view.id,
            false,
        )
        .await
        .unwrap_err();
    assert_eq!(err, agent_core::RoutineError::NotFound);
}
