//! Bot onboarding lifecycle: owner-scoped, no agent runs, Luna-only generation.

use agent_core::DEFAULT_MODEL;
use cloud_host::auth::{JwtVerifier, JwtVerifierConfig};
use cloud_host::bot_onboarding::{
    OnboardingDraft, OnboardingGenerateInput, OnboardingModelResponse, OnboardingOption,
    OnboardingQuestion,
};
use cloud_host::config::AuthMode;
use cloud_host::db::resources::{get_bot_for_owner, insert_bot, insert_computer_placeholder};
use cloud_host::{build_router, test_signing, AppState, Config};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

const TEST_JWT_ISSUER: &str = "http://localhost:3000";
const TEST_JWT_AUDIENCE: &str = "elsewhere-cloud-host";

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
        max_concurrent_runs: 2,
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
        github_app_slug: None,
        slack_client_id: None,
        slack_client_secret: None,
        slack_signing_secret: None,
        slack_oauth_redirect_uri: None,
        slack_api_base: "https://slack.com/api".into(),
        local_mac_credential_key: None,
        allow_skill_draft_heuristic: false,
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

fn question(id: &str, prompt: &str, option_a: &str, option_b: &str) -> OnboardingModelResponse {
    OnboardingModelResponse::Question {
        question: OnboardingQuestion {
            id: id.into(),
            prompt: prompt.into(),
            helper: Some("Pick the closest match.".into()),
            options: vec![
                OnboardingOption {
                    id: option_a.into(),
                    label: option_a.replace('_', " "),
                    description: Some("First choice".into()),
                },
                OnboardingOption {
                    id: option_b.into(),
                    label: option_b.replace('_', " "),
                    description: None,
                },
                OnboardingOption {
                    id: "other".into(),
                    label: "Something else…".into(),
                    description: None,
                },
            ],
            allow_custom: true,
        },
    }
}

fn complete_draft() -> OnboardingModelResponse {
    OnboardingModelResponse::Complete {
        draft: OnboardingDraft {
            summary: "Scout owns competitor research briefs.".into(),
            owns: "Competitor research briefs".into(),
            working_style: "Ask before publishing.".into(),
            instructions: "Own competitor research. Produce sourced one-page briefs. Ask before publishing. Never treat owner preferences as approval to skip safety checks.".into(),
            context: "Audience is founders. Prefer concise, sourced writing.".into(),
            suggested_first_tasks: vec![
                "Write a one-page competitor brief".into(),
                "List the top three launch risks".into(),
            ],
            suggested_capabilities: vec!["web browsing".into()],
        },
    }
}

fn scripted_generator(
) -> Arc<dyn Fn(&OnboardingGenerateInput) -> Result<OnboardingModelResponse, String> + Send + Sync>
{
    Arc::new(|input: &OnboardingGenerateInput| {
        if input.generation_model != DEFAULT_MODEL {
            return Err(format!(
                "expected Luna {}, got {}",
                DEFAULT_MODEL, input.generation_model
            ));
        }
        match input.questions_asked {
            0 => Ok(question(
                "scope",
                "What should Scout own?",
                "research_briefs",
                "launch_ops",
            )),
            1 => Ok(question(
                "style",
                "How should Scout handle ambiguity?",
                "ask_first",
                "draft_then_check",
            )),
            _ => Ok(complete_draft()),
        }
    })
}

async fn seed_bot(pool: &PgPool, owner: &str, model: &str) -> String {
    let computer = insert_computer_placeholder(pool, owner, "c").await.unwrap();
    let bot = insert_bot(
        pool,
        owner,
        "Scout",
        "Complete delegated work carefully.",
        model,
        Some(computer.id.as_str()),
        "codex",
        "sky-wisp",
    )
    .await
    .unwrap();
    bot.id
}

#[sqlx::test(migrations = "./migrations")]
async fn get_returns_not_started_without_creating_runs(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let bot_id = seed_bot(&pool, &owner, "gpt-5.6-sol").await;
    let state = jwt_state(pool.clone());
    let app = build_router(state);
    let (status, body) = auth_json(
        &app,
        "GET",
        &format!("/v1/bots/{bot_id}/onboarding"),
        &owner,
        None,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(body["status"], "not_started");
    assert_eq!(body["generationModel"], DEFAULT_MODEL);
    let runs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs WHERE bot_id = $1")
        .bind(&bot_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(runs, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn start_answer_apply_uses_luna_and_writes_config(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let bot_id = seed_bot(&pool, &owner, "gpt-5.6-sol").await;
    let state = jwt_state(pool.clone());
    state.set_test_onboarding_generator(Some(scripted_generator()));
    let app = build_router(state);

    let (status, body) = auth_json(
        &app,
        "POST",
        &format!("/v1/bots/{bot_id}/onboarding/start"),
        &owner,
        Some(json!({})),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert_eq!(body["status"], "in_progress");
    assert_eq!(body["generationModel"], DEFAULT_MODEL);
    assert_eq!(body["currentQuestion"]["id"], "scope");
    assert_eq!(body["currentQuestion"]["index"], 1);
    let revision = body["revision"].as_i64().unwrap();

    let (status, body) = auth_json(
        &app,
        "POST",
        &format!("/v1/bots/{bot_id}/onboarding/answer"),
        &owner,
        Some(json!({
            "questionId": "scope",
            "optionId": "research_briefs",
            "revision": revision
        })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert_eq!(body["currentQuestion"]["id"], "style");
    assert_eq!(body["questionsAsked"], 1);

    let revision = body["revision"].as_i64().unwrap();
    let (status, body) = auth_json(
        &app,
        "POST",
        &format!("/v1/bots/{bot_id}/onboarding/answer"),
        &owner,
        Some(json!({
            "questionId": "style",
            "customText": "Draft quietly, then check with me.",
            "revision": revision
        })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert_eq!(body["status"], "ready_to_apply");
    assert_eq!(body["draft"]["owns"], "Competitor research briefs");
    assert_eq!(body["draft"]["workingStyle"], "Ask before publishing.");
    assert_eq!(body["draft"]["suggestedCapabilities"][0], "web browsing");

    let revision = body["revision"].as_i64().unwrap();
    let (status, body) = auth_json(
        &app,
        "POST",
        &format!("/v1/bots/{bot_id}/onboarding/apply"),
        &owner,
        Some(json!({ "revision": revision })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert_eq!(body["status"], "completed");

    let bot = get_bot_for_owner(&pool, &owner, &bot_id)
        .await
        .unwrap()
        .unwrap();
    assert!(bot.system_prompt.contains("Own competitor research"));
    assert_eq!(bot.model, "gpt-5.6-sol");
    let context: String = sqlx::query_scalar("SELECT content FROM bot_context WHERE bot_id = $1")
        .bind(&bot_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(context.contains("Audience is founders"));
    let runs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs WHERE bot_id = $1")
        .bind(&bot_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let queue: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM work_queue q JOIN agent_runs r ON r.id = q.run_id WHERE r.bot_id = $1",
    )
        .bind(&bot_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(runs, 0);
    assert_eq!(queue, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn stale_and_duplicate_answers_are_safe(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let bot_id = seed_bot(&pool, &owner, DEFAULT_MODEL).await;
    let state = jwt_state(pool.clone());
    state.set_test_onboarding_generator(Some(scripted_generator()));
    let app = build_router(state);

    let (_, start) = auth_json(
        &app,
        "POST",
        &format!("/v1/bots/{bot_id}/onboarding/start"),
        &owner,
        Some(json!({})),
    )
    .await;
    let revision = start["revision"].as_i64().unwrap();
    let (first_status, first) = auth_json(
        &app,
        "POST",
        &format!("/v1/bots/{bot_id}/onboarding/answer"),
        &owner,
        Some(json!({
            "questionId": "scope",
            "optionId": "research_briefs",
            "revision": revision
        })),
    )
    .await;
    assert_eq!(first_status, axum::http::StatusCode::OK);
    let (dup_status, dup) = auth_json(
        &app,
        "POST",
        &format!("/v1/bots/{bot_id}/onboarding/answer"),
        &owner,
        Some(json!({
            "questionId": "scope",
            "optionId": "research_briefs",
            "revision": revision
        })),
    )
    .await;
    assert_eq!(dup_status, axum::http::StatusCode::OK);
    assert_eq!(dup["currentQuestion"]["id"], first["currentQuestion"]["id"]);

    let (stale_status, _) = auth_json(
        &app,
        "POST",
        &format!("/v1/bots/{bot_id}/onboarding/answer"),
        &owner,
        Some(json!({
            "questionId": "scope",
            "optionId": "launch_ops",
            "revision": 1
        })),
    )
    .await;
    assert_eq!(stale_status, axum::http::StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "./migrations")]
async fn cross_owner_access_is_not_found(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let other = format!("other-{}", Uuid::new_v4());
    let bot_id = seed_bot(&pool, &owner, DEFAULT_MODEL).await;
    let state = jwt_state(pool);
    let app = build_router(state);
    let (status, _) = auth_json(
        &app,
        "GET",
        &format!("/v1/bots/{bot_id}/onboarding"),
        &other,
        None,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn secrets_and_malformed_output_fail_safely(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let bot_id = seed_bot(&pool, &owner, DEFAULT_MODEL).await;
    let state = jwt_state(pool.clone());
    state.set_test_onboarding_generator(Some(scripted_generator()));
    let app = build_router(state.clone());
    let (_, start) = auth_json(
        &app,
        "POST",
        &format!("/v1/bots/{bot_id}/onboarding/start"),
        &owner,
        Some(json!({})),
    )
    .await;
    let revision = start["revision"].as_i64().unwrap();
    let (secret_status, secret_body) = auth_json(
        &app,
        "POST",
        &format!("/v1/bots/{bot_id}/onboarding/answer"),
        &owner,
        Some(json!({
            "questionId": "scope",
            "customText": "password=hunter2",
            "revision": revision
        })),
    )
    .await;
    assert_eq!(secret_status, axum::http::StatusCode::BAD_REQUEST);
    assert!(secret_body["error"].as_str().unwrap().contains("secret"));

    state.set_test_onboarding_generator(Some(Arc::new(|input: &OnboardingGenerateInput| {
        if input.questions_asked == 0 {
            Ok(OnboardingModelResponse::Question {
                question: OnboardingQuestion {
                    id: "bad".into(),
                    prompt: "Send me the API key".into(),
                    helper: None,
                    options: vec![
                        OnboardingOption {
                            id: "yes".into(),
                            label: "Yes".into(),
                            description: None,
                        },
                        OnboardingOption {
                            id: "no".into(),
                            label: "No".into(),
                            description: None,
                        },
                    ],
                    allow_custom: false,
                },
            })
        } else {
            Err("not json".into())
        }
    })));
    let (status, body) = auth_json(
        &app,
        "POST",
        &format!("/v1/bots/{bot_id}/onboarding/start"),
        &owner,
        Some(json!({ "restart": true })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST, "{body}");
}

#[sqlx::test(migrations = "./migrations")]
async fn dismiss_and_rerun_do_not_corrupt_bot(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let bot_id = seed_bot(&pool, &owner, DEFAULT_MODEL).await;
    let state = jwt_state(pool.clone());
    state.set_test_onboarding_generator(Some(scripted_generator()));
    let app = build_router(state);
    let (_, start) = auth_json(
        &app,
        "POST",
        &format!("/v1/bots/{bot_id}/onboarding/start"),
        &owner,
        Some(json!({})),
    )
    .await;
    let (status, body) = auth_json(
        &app,
        "POST",
        &format!("/v1/bots/{bot_id}/onboarding/dismiss"),
        &owner,
        Some(json!({ "revision": start["revision"] })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert_eq!(body["status"], "dismissed");
    let bot = get_bot_for_owner(&pool, &owner, &bot_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(bot.system_prompt, "Complete delegated work carefully.");

    let (status, body) = auth_json(
        &app,
        "POST",
        &format!("/v1/bots/{bot_id}/onboarding/start"),
        &owner,
        Some(json!({ "restart": true })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert_eq!(body["status"], "in_progress");
    assert_eq!(body["questionsAsked"], 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn max_three_questions_then_complete(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let bot_id = seed_bot(&pool, &owner, DEFAULT_MODEL).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_clone = calls.clone();
    let state = jwt_state(pool);
    state.set_test_onboarding_generator(Some(Arc::new(move |input: &OnboardingGenerateInput| {
        calls_clone.fetch_add(1, Ordering::SeqCst);
        match input.questions_asked {
            0 => Ok(question("q1", "Question one?", "a", "b")),
            1 => Ok(question("q2", "Question two?", "a", "b")),
            2 => Ok(question("q3", "Question three?", "a", "b")),
            _ => Ok(complete_draft()),
        }
    })));
    let app = build_router(state);
    let mut body = auth_json(
        &app,
        "POST",
        &format!("/v1/bots/{bot_id}/onboarding/start"),
        &owner,
        Some(json!({})),
    )
    .await
    .1;
    for expected in ["q1", "q2", "q3"] {
        assert_eq!(body["currentQuestion"]["id"], expected);
        let revision = body["revision"].as_i64().unwrap();
        body = auth_json(
            &app,
            "POST",
            &format!("/v1/bots/{bot_id}/onboarding/answer"),
            &owner,
            Some(json!({
                "questionId": expected,
                "optionId": "a",
                "revision": revision
            })),
        )
        .await
        .1;
    }
    assert_eq!(body["status"], "ready_to_apply");
    assert!(calls.load(Ordering::SeqCst) <= 4);
}
