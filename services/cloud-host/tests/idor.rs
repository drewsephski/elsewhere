use cloud_host::auth::jwt_test::test_signing::{self, TEST_KID};
use cloud_host::auth::{JwtVerifier, JwtVerifierConfig};
use cloud_host::config::{AuthMode, Config};
use cloud_host::db::resources::insert_computer_placeholder;
use cloud_host::{build_router, AppState};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

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

fn jwt_state(pool: PgPool) -> AppState {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://elsewhere:elsewhere@127.0.0.1:5432/elsewhere".into());
    let config = Config {
        database_url,
        openai_api_key: Some("test-key".into()),
        sprite_token: "test-sprite".into(),
        api_token: "test-token".into(),
        auth_mode: AuthMode::Jwt,
        jwt_issuer: Some("http://localhost:3000".into()),
        jwt_audience: Some("elsewhere-cloud-host".into()),
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
        tool_approval_timeout_secs: 300,
        enforce_tool_approvals_internal: false,
    };
    let mut state = AppState::new(pool, config);
    state.jwt_verifier = Some(JwtVerifier::from_test_decoding_key(
        TEST_KID,
        test_signing::verifier(),
        JwtVerifierConfig {
            jwks_url: "http://127.0.0.1:9/jwks".into(),
            issuer: "http://localhost:3000".into(),
            audience: "elsewhere-cloud-host".into(),
        },
    ));
    state
}

fn token(sub: &str) -> String {
    test_signing::user_token(sub, "http://localhost:3000", "elsewhere-cloud-host", 300)
}

#[tokio::test]
async fn user_cannot_read_other_users_bot() {
    let Some(pool) = try_test_pool().await else {
        return;
    };
    let computer = insert_computer_placeholder(&pool, "user-a", "A computer")
        .await
        .unwrap();
    let bot = cloud_host::db::resources::insert_bot(
        &pool,
        "user-a",
        "Bot A",
        "instructions",
        "gpt-5.6-luna",
        Some(computer.id.as_str()),
        "auto",
    )
    .await
    .unwrap();

    let app = build_router(jwt_state(pool));
    let resp = app
        .oneshot(
            http::Request::builder()
                .uri(format!("/v1/bots/{}", bot.id))
                .header("authorization", format!("Bearer {}", token("user-b")))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn user_cannot_delete_other_users_bot() {
    let Some(pool) = try_test_pool().await else {
        return;
    };
    let bot = cloud_host::db::resources::insert_bot(
        &pool,
        "user-a",
        "Bot A",
        "i",
        "gpt-5.6-luna",
        None,
        "auto",
    )
    .await
    .unwrap();
    let app = build_router(jwt_state(pool));
    let resp = app
        .oneshot(
            http::Request::builder()
                .method("DELETE")
                .uri(format!("/v1/bots/{}", bot.id))
                .header("authorization", format!("Bearer {}", token("user-b")))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn user_cannot_stream_other_users_run_events() {
    let Some(pool) = try_test_pool().await else {
        return;
    };
    let run_id = Uuid::new_v4().to_string();
    let request_id = Uuid::new_v4().to_string();
    let bot_id = Uuid::new_v4().to_string();
    let conv_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO bots (id, owner_id, name, system_prompt, model, engine_preference) VALUES ($1, 'user-a', 'a', '', 'gpt-5.6-luna', 'auto')",
    )
    .bind(&bot_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO conversations (id, owner_id, bot_id) VALUES ($1, 'user-a', $2)")
        .bind(&conv_id)
        .bind(&bot_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        r#"
        INSERT INTO agent_runs (
            id, owner_id, request_id, bot_id, conversation_id, model, status, step_count, created_at, updated_at
        ) VALUES ($1, 'user-a', $2, $3, $4, 'gpt-5.6-luna', 'complete', 0, NOW(), NOW())
        "#,
    )
    .bind(&run_id)
    .bind(&request_id)
    .bind(&bot_id)
    .bind(&conv_id)
    .execute(&pool)
    .await
    .unwrap();

    let app = build_router(jwt_state(pool));
    let resp = app
        .oneshot(
            http::Request::builder()
                .uri(format!("/v1/runs/{run_id}/events"))
                .header("authorization", format!("Bearer {}", token("user-b")))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn provider_status_requires_auth_and_serializes() {
    let Some(pool) = try_test_pool().await else {
        return;
    };
    let app = build_router(jwt_state(pool.clone()));
    let anon = app
        .clone()
        .oneshot(
            http::Request::builder()
                .uri("/v1/providers/status")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(anon.status(), http::StatusCode::UNAUTHORIZED);

    let ok = app
        .oneshot(
            http::Request::builder()
                .uri("/v1/providers/status")
                .header("authorization", format!("Bearer {}", token("user-a")))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ok.status(), http::StatusCode::OK);
    let body = axum::body::to_bytes(ok.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json.get("codexInstalled").is_some());
    assert!(json.get("defaultModel").is_some());
    assert!(json.get("chatgptConnected").is_some());
}

#[tokio::test]
async fn provider_profiles_are_persistent_private_and_owner_scoped() {
    let pool = try_test_pool()
        .await
        .expect("test Postgres must be running");
    let mut config = (*jwt_state(pool.clone()).config).clone();
    let root = std::env::temp_dir().join(format!("elsewhere-profiles-test-{}", Uuid::new_v4()));
    config.codex_profiles_dir = Some(root.clone());
    let owner_a = format!("../user-a-{}", Uuid::new_v4());
    let owner_b = format!("user-b-{}", Uuid::new_v4());
    let a = cloud_host::provider_profile::profile_for_owner(&pool, &config, &owner_a)
        .await
        .unwrap()
        .unwrap();
    let again = cloud_host::provider_profile::profile_for_owner(&pool, &config, &owner_a)
        .await
        .unwrap()
        .unwrap();
    let b = cloud_host::provider_profile::profile_for_owner(&pool, &config, &owner_b)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(a, again);
    assert_ne!(a, b);
    assert_eq!(a.parent(), Some(root.as_path()));
    assert!(Uuid::parse_str(a.file_name().unwrap().to_str().unwrap()).is_ok());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&a).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    config.codex_profiles_dir = None;
    assert!(
        cloud_host::provider_profile::profile_for_owner(&pool, &config, &owner_a)
            .await
            .is_err()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn user_cannot_cancel_another_users_chatgpt_login() {
    use cloud_host::app_state::PendingCodexLogin;
    let pool = try_test_pool()
        .await
        .expect("test Postgres must be running");
    let mut state = jwt_state(pool);
    let mut config = (*state.config).clone();
    config.allow_codex_login = true;
    state.config = std::sync::Arc::new(config);
    let client = codex_provider::CodexAppServerClient::from_process(
        codex_provider::spawn_fake_app_server().await.unwrap(),
    )
    .await
    .unwrap();
    *state.codex_login_client.lock().await = Some(PendingCodexLogin {
        owner_id: "user-a".into(),
        login_id: "private-login".into(),
        auth_url: "https://auth.openai.com/codex/device".into(),
        user_code: "secret-code".into(),
        expires_at: std::time::Instant::now() + std::time::Duration::from_secs(600),
        client: std::sync::Arc::new(client),
    });
    let response = build_router(state.clone())
        .oneshot(
            http::Request::builder()
                .method("POST")
                .uri("/v1/providers/codex/login/cancel")
                .header("authorization", format!("Bearer {}", token("user-b")))
                .header("content-type", "application/json")
                .body(axum::body::Body::from(r#"{"loginId":"private-login"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), http::StatusCode::NOT_FOUND);
    assert!(state.codex_login_client.lock().await.is_some());
}

#[tokio::test]
async fn jwt_cannot_impersonate_trusted_local_runner() {
    let pool = try_test_pool()
        .await
        .expect("test Postgres must be running");
    let response = build_router(jwt_state(pool))
        .oneshot(
            http::Request::builder()
                .uri("/v1/providers/status")
                .header("authorization", format!("Bearer {}", token("legacy-local")))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), http::StatusCode::UNAUTHORIZED);
}
