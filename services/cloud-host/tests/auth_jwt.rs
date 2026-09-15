//! JWT verification tests using a static ES256 key pair (no network).

use cloud_host::auth::{JwtVerifier, JwtVerifierConfig, LEGACY_LOCAL_OWNER};
use cloud_host::config::{AuthMode, Config};
use cloud_host::{build_router, test_signing, AppState};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

const TEST_JWT_ISSUER: &str = "http://localhost:3000";
const TEST_JWT_AUDIENCE: &str = "elsewhere-cloud-host";

fn jwt_config() -> JwtVerifierConfig {
    JwtVerifierConfig {
        jwks_url: "http://127.0.0.1:9/jwks".into(),
        issuer: TEST_JWT_ISSUER.into(),
        audience: TEST_JWT_AUDIENCE.into(),
    }
}

async fn require_test_pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        panic!(
            "DATABASE_URL is required for cloud-host JWT integration tests (set postgres URL or use sqlx::test harness)"
        );
    });
    let pool = tokio::time::timeout(Duration::from_secs(5), PgPool::connect(&url))
        .await
        .expect("database connection timed out")
        .expect("database connection failed");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations failed");
    pool
}

fn jwt_test_config() -> Config {
    Config {
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
        enforce_tool_approvals_internal: false,
        legacy_local_approval_bypass: false,
        browser_enabled: false,
        connector_secret_key: None,
        github_client_id: None,
        github_client_secret: None,
        github_oauth_redirect_uri: None,
    }
}

fn test_verifier() -> Arc<JwtVerifier> {
    JwtVerifier::from_test_decoding_key(
        test_signing::TEST_KID,
        test_signing::verifier(),
        jwt_config(),
    )
}

fn token(sub: &str, exp_offset_secs: i64) -> String {
    test_signing::user_token(sub, TEST_JWT_ISSUER, TEST_JWT_AUDIENCE, exp_offset_secs)
}

#[tokio::test]
async fn jwt_verifier_accepts_valid_token() {
    let verifier = test_verifier();
    let sub = verifier
        .verify_bearer_token(&token("user-a", 300))
        .await
        .unwrap();
    assert_eq!(sub, "user-a");
}

#[tokio::test]
async fn jwt_verifier_rejects_expired_and_bad_audience() {
    let verifier = test_verifier();
    assert!(verifier.verify_bearer_token(&token("user-a", -3600)).await.is_err());
    let bad_aud =
        test_signing::user_token("user-a", TEST_JWT_ISSUER, "wrong-audience", 300);
    assert!(verifier.verify_bearer_token(&bad_aud).await.is_err());
}

#[tokio::test]
async fn jwt_auth_protects_bot_listing() {
    let pool = require_test_pool().await;
    let mut state = AppState::new(pool, jwt_test_config());
    state.jwt_verifier = Some(test_verifier());
    let app = build_router(state);

    let unauthorized = app
        .clone()
        .oneshot(
            http::Request::builder()
                .uri("/v1/bots")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), http::StatusCode::UNAUTHORIZED);

    let ok = app
        .oneshot(
            http::Request::builder()
                .uri("/v1/bots")
                .header("authorization", format!("Bearer {}", token("user-a", 300)))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ok.status(), http::StatusCode::OK);
}

#[tokio::test]
async fn hybrid_internal_token_still_maps_legacy_local() {
    let pool = require_test_pool().await;
    let mut config = jwt_test_config();
    config.auth_mode = AuthMode::Hybrid;
    let state = AppState::new(pool, config);
    let app = build_router(state);
    let legacy = app
        .oneshot(
            http::Request::builder()
                .uri("/v1/bots")
                .header("authorization", "Bearer test-token")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(legacy.status(), http::StatusCode::OK);
    let _ = LEGACY_LOCAL_OWNER;
}
