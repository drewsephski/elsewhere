//! Bot-facing skill tools: draft, approval, attachment, and isolation.

use agent_core::{AgentSkills, CreateResponseResult, ModelError, ResponsesModel};
use async_trait::async_trait;
use cloud_host::auth::{JwtVerifier, JwtVerifierConfig};
use cloud_host::config::{AuthMode, Config};
use cloud_host::db::resources::{insert_bot, insert_computer_placeholder};
use cloud_host::skills::{self, SkillAdmissionInput};
use cloud_host::{build_router, test_signing, AppState, TestRunOverrides};
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
        "responses",
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

async fn seed_completed_run(
    pool: &PgPool,
    owner: &str,
    bot_id: &str,
    task: &str,
    answer: &str,
) -> (String, String) {
    let admission = SkillAdmissionInput::default();
    let mut tx = pool.begin().await.unwrap();
    let records = cloud_host::work::enqueue_in_transaction(
        &mut tx,
        owner,
        &Uuid::new_v4().to_string(),
        bot_id,
        None,
        task,
        &admission,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    sqlx::query(
        "UPDATE messages SET body = $2, status = 'complete' WHERE id = $1",
    )
    .bind(&records.assistant_message_id)
    .bind(answer)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE agent_runs SET status = 'completed', finished_at = NOW(), updated_at = NOW() WHERE id = $1",
    )
    .bind(&records.run_id)
    .execute(pool)
    .await
    .unwrap();
    (records.run_id, records.conversation_id)
}

async fn start_run(
    state: &AppState,
    owner: &str,
    bot_id: &str,
    message: &str,
    request_id: &str,
    conversation_id: Option<&str>,
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
    let mut body = json!({ "botId": bot_id, "message": message });
    if let Some(id) = conversation_id {
        body["conversationId"] = json!(id);
    }
    let response = app
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
    assert_eq!(response.status(), axum::http::StatusCode::ACCEPTED);
    cloud_host::worker::dispatch_available(state).await.unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn skill_list_includes_attachment_state(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let md = "---\nname: list-test\ndescription: d\n---\n\nbody\n";
    let package = agent_skills::SkillPackage::validate_and_build(md, &[], Some("list-test")).unwrap();
    let (skill, _) =
        skills::create_skill_with_version(&pool, &owner, "list-test", &package, &[])
            .await
            .unwrap();
    skills::attach_bot_skill(&pool, &owner, &bot_id, &skill.id, None)
        .await
        .unwrap();

    let service = cloud_host::agent_skills::PostgresAgentSkills::new(
        pool.clone(),
        jwt_state(pool.clone()).config.as_ref().clone(),
    );
    let rows = service
        .list(&agent_core::SkillContext {
            owner_id: owner.clone(),
            bot_id: bot_id.clone(),
            source_conversation_id: "conv".into(),
        })
        .await
        .unwrap();
    assert!(rows.iter().any(|row| row.slug == "list-test" && row.attached_to_bot));
}

#[sqlx::test(migrations = "./migrations")]
async fn skill_save_requires_approval_and_attaches(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let (_prior_run, conversation_id) =
        seed_completed_run(&pool, &owner, &bot_id, "Research competitors", "Brief ready.").await;
    let state = jwt_state(pool.clone());
    let request_id = Uuid::new_v4().to_string();
    let args = json!({
        "name": "Competitor brief",
        "description": "Weekly competitor research",
        "attachToBot": true
    });
    start_run(
        &state,
        &owner,
        &bot_id,
        "save that as a skill",
        &request_id,
        Some(&conversation_id),
        ScriptedModel::calls("skill_save_recent_work", &args.to_string()),
    )
    .await;
    let approval_id = wait_pending_approval(&pool, &owner).await;
    let pending: (String,) = sqlx::query_as(
        "SELECT tool_name FROM tool_approval_requests WHERE id = $1",
    )
    .bind(&approval_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pending.0, "skill_save_recent_work");
    let app = build_router(state.clone());
    let approve = app
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
    cloud_host::worker::dispatch_available(&state).await.unwrap();
    wait_no_active_runs(&pool, &owner).await;
    state.clear_test_run_overrides(&request_id);

    let rows = skills::list_skills(&pool, &owner).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].slug, "competitor-brief");
    let attached = skills::list_bot_skills(&pool, &owner, &bot_id)
        .await
        .unwrap();
    assert_eq!(attached.len(), 1);
    assert_eq!(attached[0].slug, "competitor-brief");
}

#[sqlx::test(migrations = "./migrations")]
async fn denied_skill_save_persists_nothing(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let (_, conversation_id) =
        seed_completed_run(&pool, &owner, &bot_id, "Task", "Done").await;
    let state = jwt_state(pool.clone());
    let request_id = Uuid::new_v4().to_string();
    start_run(
        &state,
        &owner,
        &bot_id,
        "save skill",
        &request_id,
        Some(&conversation_id),
        ScriptedModel::calls("skill_save_recent_work", "{}"),
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
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deny.status(), axum::http::StatusCode::OK);
    cloud_host::worker::dispatch_available(&state).await.unwrap();
    wait_no_active_runs(&pool, &owner).await;
    state.clear_test_run_overrides(&request_id);
    assert!(skills::list_skills(&pool, &owner).await.unwrap().is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn skill_save_uses_prior_run_not_current(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let (_, conversation_id) =
        seed_completed_run(&pool, &owner, &bot_id, "First task", "First answer").await;
    let state = jwt_state(pool.clone());
    let request_id = Uuid::new_v4().to_string();
    start_run(
        &state,
        &owner,
        &bot_id,
        "save skill",
        &request_id,
        Some(&conversation_id),
        ScriptedModel::calls(
            "skill_save_recent_work",
            r#"{"name":"From first task"}"#,
        ),
    )
    .await;
    let approval_id = wait_pending_approval(&pool, &owner).await;
    let args: serde_json::Value = sqlx::query_scalar(
        "SELECT arguments_json FROM tool_approval_requests WHERE id = $1",
    )
    .bind(&approval_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(args.get("name").and_then(|v| v.as_str()), Some("From first task"));
}

#[sqlx::test(migrations = "./migrations")]
async fn skill_save_without_prior_completed_run_fails(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let state = jwt_state(pool.clone());
    let request_id = Uuid::new_v4().to_string();
    start_run(
        &state,
        &owner,
        &bot_id,
        "save skill",
        &request_id,
        None,
        ScriptedModel::calls("skill_save_recent_work", "{}"),
    )
    .await;
    wait_no_active_runs(&pool, &owner).await;
    state.clear_test_run_overrides(&request_id);
    assert_eq!(pending_approval_count(&pool, &owner).await, 0);
    assert!(skills::list_skills(&pool, &owner).await.unwrap().is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn duplicate_slug_is_conflict(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let md = "---\nname: competitor-brief\ndescription: existing\n---\n\nx\n";
    let package =
        agent_skills::SkillPackage::validate_and_build(md, &[], Some("competitor-brief")).unwrap();
    skills::create_skill_with_version(&pool, &owner, "competitor-brief", &package, &[])
        .await
        .unwrap();
    let (_, conversation_id) =
        seed_completed_run(&pool, &owner, &bot_id, "Task", "Done").await;
    let state = jwt_state(pool.clone());
    let request_id = Uuid::new_v4().to_string();
    start_run(
        &state,
        &owner,
        &bot_id,
        "save",
        &request_id,
        Some(&conversation_id),
        ScriptedModel::calls(
            "skill_save_recent_work",
            r#"{"name":"Competitor brief"}"#,
        ),
    )
    .await;
    wait_no_active_runs(&pool, &owner).await;
    state.clear_test_run_overrides(&request_id);
    assert_eq!(pending_approval_count(&pool, &owner).await, 0);
    assert_eq!(skills::list_skills(&pool, &owner).await.unwrap().len(), 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn attach_and_detach_require_approval(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let md = "---\nname: attach-me\ndescription: d\n---\n\nb\n";
    let package = agent_skills::SkillPackage::validate_and_build(md, &[], Some("attach-me")).unwrap();
    let (skill, _) =
        skills::create_skill_with_version(&pool, &owner, "attach-me", &package, &[])
            .await
            .unwrap();
    let state = jwt_state(pool.clone());
    let attach_req = Uuid::new_v4().to_string();
    start_run(
        &state,
        &owner,
        &bot_id,
        "attach",
        &attach_req,
        None,
        ScriptedModel::calls(
            "skill_attach",
            &json!({ "skillId": skill.id }).to_string(),
        ),
    )
    .await;
    let attach_approval = wait_pending_approval(&pool, &owner).await;
    let app = build_router(state.clone());
    app.clone()
        .oneshot(
        axum::http::Request::builder()
            .method("POST")
            .uri(format!("/v1/approvals/{attach_approval}/approve"))
            .header("Authorization", format!("Bearer {}", token(&owner)))
            .body(axum::body::Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();
    cloud_host::worker::dispatch_available(&state).await.unwrap();
    wait_no_active_runs(&pool, &owner).await;
    state.clear_test_run_overrides(&attach_req);
    assert_eq!(
        skills::list_bot_skills(&pool, &owner, &bot_id)
            .await
            .unwrap()
            .len(),
        1
    );

    let detach_req = Uuid::new_v4().to_string();
    start_run(
        &state,
        &owner,
        &bot_id,
        "detach",
        &detach_req,
        None,
        ScriptedModel::calls(
            "skill_detach",
            &json!({ "skillId": skill.id }).to_string(),
        ),
    )
    .await;
    let detach_approval = wait_pending_approval(&pool, &owner).await;
    app.oneshot(
        axum::http::Request::builder()
            .method("POST")
            .uri(format!("/v1/approvals/{detach_approval}/approve"))
            .header("Authorization", format!("Bearer {}", token(&owner)))
            .body(axum::body::Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();
    cloud_host::worker::dispatch_available(&state).await.unwrap();
    wait_no_active_runs(&pool, &owner).await;
    state.clear_test_run_overrides(&detach_req);
    assert!(
        skills::list_bot_skills(&pool, &owner, &bot_id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn archived_skill_attach_rejected(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let md = "---\nname: archived-skill\ndescription: d\n---\n\nb\n";
    let package =
        agent_skills::SkillPackage::validate_and_build(md, &[], Some("archived-skill")).unwrap();
    let (skill, _) =
        skills::create_skill_with_version(&pool, &owner, "archived-skill", &package, &[])
            .await
            .unwrap();
    skills::patch_skill_metadata(&pool, &owner, &skill.id, None, None, Some("archived"))
        .await
        .unwrap();
    let service = cloud_host::agent_skills::PostgresAgentSkills::new(
        pool.clone(),
        jwt_state(pool.clone()).config.as_ref().clone(),
    );
    let err = service
        .attach(
            &agent_core::SkillContext {
                owner_id: owner.clone(),
                bot_id: bot_id.clone(),
                source_conversation_id: "c".into(),
            },
            &skill.id,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, agent_core::SkillError::Validation(_)));
}

#[sqlx::test(migrations = "./migrations")]
async fn cross_owner_skill_attach_denied(pool: PgPool) {
    let owner_a = format!("owner-{}", Uuid::new_v4());
    let owner_b = format!("owner-{}", Uuid::new_v4());
    let (bot_b, _) = seed_bot(&pool, &owner_b).await;
    let md = "---\nname: secret\ndescription: d\n---\n\nb\n";
    let package = agent_skills::SkillPackage::validate_and_build(md, &[], Some("secret")).unwrap();
    let (skill, _) = skills::create_skill_with_version(&pool, &owner_a, "secret", &package, &[])
        .await
        .unwrap();
    let service = cloud_host::agent_skills::PostgresAgentSkills::new(
        pool.clone(),
        jwt_state(pool.clone()).config.as_ref().clone(),
    );
    let err = service
        .attach(
            &agent_core::SkillContext {
                owner_id: owner_b.clone(),
                bot_id: bot_b.clone(),
                source_conversation_id: "c".into(),
            },
            &skill.id,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, agent_core::SkillError::NotFound));
}
