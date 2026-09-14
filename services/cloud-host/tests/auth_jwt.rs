//! JWT verification tests using a static ES256 key pair (no network).

use cloud_host::auth::jwt_test::test_signing::{self, TEST_KID};
use cloud_host::auth::{JwtVerifier, JwtVerifierConfig, LEGACY_LOCAL_OWNER};
use cloud_host::config::{AuthMode, Config};
use cloud_host::{build_router, AppState};
use sqlx::PgPool;
use tower::ServiceExt;

fn jwt_config() -> JwtVerifierConfig {
    JwtVerifierConfig {
        jwks_url: "http://127.0.0.1:9/jwks".into(),
        issuer: "http://localhost:3000".into(),
        audience: "elsewhere-cloud-host".into(),
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

fn jwt_test_config() -> Config {
    Config {
        database_url: std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://elsewhere:elsewhere@127.0.0.1:5432/elsewhere".into()),
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
        browser_enabled: false,
    }
}

fn test_verifier() -> std::sync::Arc<JwtVerifier> {
    JwtVerifier::from_test_decoding_key(TEST_KID, test_signing::verifier(), jwt_config())
}

#[tokio::test]
async fn jwt_verifier_accepts_valid_token() {
    let verifier = test_verifier();
    let token = test_signing::user_token(
        "user-a",
        "http://localhost:3000",
        "elsewhere-cloud-host",
        300,
    );
    let sub = verifier.verify_bearer_token(&token).await.unwrap();
    assert_eq!(sub, "user-a");
}

#[tokio::test]
async fn jwt_verifier_rejects_expired_and_bad_audience() {
    let verifier = test_verifier();
    let expired = test_signing::user_token(
        "user-a",
        "http://localhost:3000",
        "elsewhere-cloud-host",
        -3600,
    );
    assert!(verifier.verify_bearer_token(&expired).await.is_err());
    let bad_aud =
        test_signing::user_token("user-a", "http://localhost:3000", "wrong-audience", 300);
    assert!(verifier.verify_bearer_token(&bad_aud).await.is_err());
}

#[tokio::test]
async fn jwt_auth_protects_bot_listing() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping jwt_auth_protects_bot_listing: Postgres unavailable");
        return;
    };
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

    let token = test_signing::user_token(
        "user-a",
        "http://localhost:3000",
        "elsewhere-cloud-host",
        300,
    );
    let ok = app
        .oneshot(
            http::Request::builder()
                .uri("/v1/bots")
                .header("authorization", format!("Bearer {token}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ok.status(), http::StatusCode::OK);
}

#[tokio::test]
async fn hybrid_internal_token_still_maps_legacy_local() {
    let Some(pool) = try_test_pool().await else {
        return;
    };
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
