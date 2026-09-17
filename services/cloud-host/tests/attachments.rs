//! Attachment upload, admission, isolation, staging, and memory-safety coverage.

use cloud_host::attachments::store;
use cloud_host::attachments::validate::validate_upload_bytes;
use cloud_host::auth::{JwtVerifier, JwtVerifierConfig};
use cloud_host::config::AuthMode;
use cloud_host::db::resources::{insert_bot, insert_computer_placeholder};
use cloud_host::error::ApiError;
use cloud_host::skills::SkillAdmissionInput;
use cloud_host::work;
use cloud_host::Config;
use cloud_host::{build_router, test_signing, AppState};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
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

async fn bot(pool: &PgPool, owner: &str) -> cloud_host::db::resources::BotRow {
    let computer = insert_computer_placeholder(pool, owner, "Computer")
        .await
        .unwrap();
    insert_bot(
        pool,
        owner,
        "Scout",
        "Help",
        "gpt-5.6-luna",
        Some(computer.id.as_str()),
        "responses",
        "sky-wisp",
    )
    .await
    .unwrap()
}

fn png_bytes() -> Vec<u8> {
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    png.extend_from_slice(&[0; 24]);
    png
}

async fn stage(
    pool: &PgPool,
    owner: &str,
    bot_id: &str,
    name: &str,
    mime: &str,
    bytes: &[u8],
) -> String {
    let validated = validate_upload_bytes(name, Some(mime), bytes).unwrap();
    store::insert_staged(pool, owner, Some(bot_id), None, &validated, bytes)
        .await
        .unwrap()
        .id
}

fn multipart(name: &str, mime: &str, bytes: &[u8]) -> (String, Vec<u8>) {
    let boundary = "----elsewhereTestBoundary";
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: {mime}\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

#[sqlx::test(migrations = "./migrations")]
async fn upload_validates_types_and_owner_isolation(pool: PgPool) {
    let alice = format!("alice-{}", Uuid::new_v4());
    let bob = format!("bob-{}", Uuid::new_v4());
    let alice_bot = bot(&pool, &alice).await;
    let bob_bot = bot(&pool, &bob).await;
    let state = jwt_state(pool.clone());
    let app = build_router(state);

    let (content_type, body) = multipart("note.txt", "text/plain", b"hello world");
    let created = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/v1/bots/{}/attachments", alice_bot.id))
                .header("Authorization", format!("Bearer {}", token(&alice)))
                .header("content-type", content_type)
                .body(axum::body::Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), axum::http::StatusCode::CREATED);
    let payload: Value =
        serde_json::from_slice(&created.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let id = payload["id"].as_str().unwrap();

    let (zip_type, zip_body) = multipart("x.zip", "application/zip", b"PK\x03\x04rest");
    let rejected = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/v1/bots/{}/attachments", alice_bot.id))
                .header("Authorization", format!("Bearer {}", token(&alice)))
                .header("content-type", zip_type)
                .body(axum::body::Body::from(zip_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected.status(), axum::http::StatusCode::BAD_REQUEST);

    let foreign = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri(format!("/v1/attachments/{id}"))
                .header("Authorization", format!("Bearer {}", token(&bob)))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(foreign.status(), axum::http::StatusCode::NOT_FOUND);

    let other_bot = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/v1/bots/{}/attachments", bob_bot.id))
                .header("Authorization", format!("Bearer {}", token(&alice)))
                .header("content-type", "multipart/form-data; boundary=x")
                .body(axum::body::Body::from("--x--"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(other_bot.status(), axum::http::StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn admission_snapshots_attachments_and_idempotency(pool: PgPool) {
    let owner = format!("own-{}", Uuid::new_v4());
    let bot = bot(&pool, &owner).await;
    let png = png_bytes();
    let a = stage(&pool, &owner, &bot.id, "a.png", "image/png", &png).await;
    let b = stage(&pool, &owner, &bot.id, "notes.txt", "text/plain", b"hello").await;
    let key = Uuid::new_v4().to_string();
    let first = work::enqueue_with_skills_and_attachments(
        &pool,
        &owner,
        &key,
        &bot.id,
        None,
        "Look at this",
        &SkillAdmissionInput::default(),
        &[a.clone(), b.clone()],
    )
    .await
    .unwrap();
    let retry = work::enqueue_with_skills_and_attachments(
        &pool,
        &owner,
        &key,
        &bot.id,
        None,
        "Look at this",
        &SkillAdmissionInput::default(),
        &[a.clone(), b.clone()],
    )
    .await
    .unwrap();
    assert_eq!(first.run_id, retry.run_id);

    let conflict = work::enqueue_with_skills_and_attachments(
        &pool,
        &owner,
        &key,
        &bot.id,
        None,
        "Look at this",
        &SkillAdmissionInput::default(),
        &[b.clone(), a.clone()],
    )
    .await;
    assert!(matches!(conflict, Err(ApiError::Conflict(_))));

    let listed = store::list_for_run(&pool, &first.run_id).await.unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].id, a);
    assert!(listed[0]
        .workspace_path
        .starts_with(&format!("/workspace/inputs/{}/", first.run_id)));

    let only = stage(&pool, &owner, &bot.id, "solo.txt", "text/plain", b"solo").await;
    let attachment_only = work::enqueue_with_skills_and_attachments(
        &pool,
        &owner,
        &Uuid::new_v4().to_string(),
        &bot.id,
        None,
        "  ",
        &SkillAdmissionInput::default(),
        &[only],
    )
    .await
    .unwrap();
    assert_eq!(
        store::list_for_run(&pool, &attachment_only.run_id)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn admission_rejects_foreign_expired_and_count_limits(pool: PgPool) {
    let alice = format!("a-{}", Uuid::new_v4());
    let bob = format!("b-{}", Uuid::new_v4());
    let alice_bot = bot(&pool, &alice).await;
    let bob_bot = bot(&pool, &bob).await;
    let stolen = stage(&pool, &bob, &bob_bot.id, "x.txt", "text/plain", b"nope").await;
    let err = work::enqueue_with_skills_and_attachments(
        &pool,
        &alice,
        &Uuid::new_v4().to_string(),
        &alice_bot.id,
        None,
        "hi",
        &SkillAdmissionInput::default(),
        &[stolen],
    )
    .await;
    assert!(err.is_err());

    let wrong_bot = stage(&pool, &alice, &alice_bot.id, "y.txt", "text/plain", b"mine").await;
    sqlx::query("UPDATE attachments SET bot_id = $1 WHERE id = $2")
        .bind(&bob_bot.id)
        .bind(&wrong_bot)
        .execute(&pool)
        .await
        .unwrap();
    assert!(work::enqueue_with_skills_and_attachments(
        &pool,
        &alice,
        &Uuid::new_v4().to_string(),
        &alice_bot.id,
        None,
        "hi",
        &SkillAdmissionInput::default(),
        &[wrong_bot],
    )
    .await
    .is_err());

    let expired = stage(
        &pool,
        &alice,
        &alice_bot.id,
        "old.txt",
        "text/plain",
        b"old",
    )
    .await;
    sqlx::query("UPDATE attachments SET expires_at = NOW() - INTERVAL '1 hour' WHERE id = $1")
        .bind(&expired)
        .execute(&pool)
        .await
        .unwrap();
    assert!(work::enqueue_with_skills_and_attachments(
        &pool,
        &alice,
        &Uuid::new_v4().to_string(),
        &alice_bot.id,
        None,
        "hi",
        &SkillAdmissionInput::default(),
        &[expired],
    )
    .await
    .is_err());

    let mut too_many = Vec::new();
    for i in 0..5 {
        too_many.push(
            stage(
                &pool,
                &alice,
                &alice_bot.id,
                &format!("n{i}.txt"),
                "text/plain",
                b"x",
            )
            .await,
        );
    }
    assert!(work::enqueue_with_skills_and_attachments(
        &pool,
        &alice,
        &Uuid::new_v4().to_string(),
        &alice_bot.id,
        None,
        "hi",
        &SkillAdmissionInput::default(),
        &too_many,
    )
    .await
    .is_err());
}

#[sqlx::test(migrations = "./migrations")]
async fn automatic_memory_source_is_typed_text_not_attachment_body(pool: PgPool) {
    let owner = format!("mem-{}", Uuid::new_v4());
    let bot = bot(&pool, &owner).await;
    let secret = b"SECRET_VENDOR_TERM_XYZ never remember this automatically";
    let attachment = stage(&pool, &owner, &bot.id, "proposal.txt", "text/plain", secret).await;
    let records = work::enqueue_with_skills_and_attachments(
        &pool,
        &owner,
        &Uuid::new_v4().to_string(),
        &bot.id,
        None,
        "Review attached vendor proposal.",
        &SkillAdmissionInput::default(),
        &[attachment],
    )
    .await
    .unwrap();
    let queued: String =
        sqlx::query_scalar("SELECT user_message FROM work_queue WHERE run_id = $1")
            .bind(&records.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(queued, "Review attached vendor proposal.");
    assert!(!queued.contains("SECRET_VENDOR_TERM_XYZ"));
}

#[sqlx::test(migrations = "./migrations")]
async fn workspace_staging_uses_safe_run_inbox(pool: PgPool) {
    let owner = format!("ws-{}", Uuid::new_v4());
    let bot = bot(&pool, &owner).await;
    let id = stage(&pool, &owner, &bot.id, "data.csv", "text/csv", b"a,b\n1,2").await;
    let records = work::enqueue_with_skills_and_attachments(
        &pool,
        &owner,
        &Uuid::new_v4().to_string(),
        &bot.id,
        None,
        "chart this",
        &SkillAdmissionInput::default(),
        &[id],
    )
    .await
    .unwrap();
    let computer = agent_core::FakeAgentComputer::new();
    let staged = cloud_host::attachments::stage_run_attachments_into_workspace(
        &computer,
        &pool,
        &records.run_id,
    )
    .await
    .unwrap();
    assert_eq!(staged.len(), 1);
    assert!(staged[0]
        .workspace_path
        .starts_with(&format!("/workspace/inputs/{}/", records.run_id)));
    assert!(!staged[0].workspace_path.contains(".."));
}
