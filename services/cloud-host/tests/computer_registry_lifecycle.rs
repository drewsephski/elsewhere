use cloud_host::auth::{JwtVerifier, JwtVerifierConfig};
use cloud_host::config::{AuthMode, Config};
use cloud_host::db::resources::{archive_computer, insert_computer_placeholder};
use cloud_host::{test_signing, AppState, ComputerRegistry};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

const TEST_JWT_ISSUER: &str = "http://localhost:3000";
const TEST_JWT_AUDIENCE: &str = "elsewhere-cloud-host";

async fn try_test_pool() -> Option<PgPool> {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://elsewhere:elsewhere@127.0.0.1:5432/elsewhere".into());
    let pool = tokio::time::timeout(Duration::from_secs(2), PgPool::connect(&url))
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
        tool_approval_timeout_secs: 300,
        enforce_tool_approvals_internal: false,
        browser_enabled: true,
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

#[tokio::test]
async fn archived_computer_evicts_registry_and_blocks_workspace_api() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping archived_computer_evicts_registry_and_blocks_workspace_api");
        return;
    };
    let owner = format!("owner-{}", Uuid::new_v4());
    let computer = insert_computer_placeholder(&pool, &owner, "Registry test")
        .await
        .unwrap();
    let state = jwt_state(pool.clone());

    let registry = ComputerRegistry::default();
    let connected = registry
        .connect_sprite(
            &state.config,
            &pool,
            &owner,
            &computer.id,
            true,
        )
        .await;
    assert!(connected.is_ok());

    archive_computer(&pool, &owner, &computer.id).await.unwrap();
    registry.evict(&owner, &computer.id);

    let reconnect = registry
        .connect_sprite(
            &state.config,
            &pool,
            &owner,
            &computer.id,
            true,
        )
        .await;
    assert!(matches!(reconnect, Err(cloud_host::error::ApiError::NotFound)));

}

#[tokio::test]
async fn registry_revalidates_owner_before_returning_cache() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping registry_revalidates_owner_before_returning_cache");
        return;
    };
    let owner_a = format!("owner-a-{}", Uuid::new_v4());
    let owner_b = format!("owner-b-{}", Uuid::new_v4());
    let computer = insert_computer_placeholder(&pool, &owner_a, "Shared id probe")
        .await
        .unwrap();
    let state = jwt_state(pool.clone());
    let registry = ComputerRegistry::default();

    registry
        .connect_sprite(&state.config, &pool, &owner_a, &computer.id, true)
        .await
        .unwrap();

    let denied = registry
        .connect_sprite(&state.config, &pool, &owner_b, &computer.id, true)
        .await;
    assert!(matches!(denied, Err(cloud_host::error::ApiError::NotFound)));
}

#[tokio::test]
async fn registry_replaces_cache_when_provider_resource_changes() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping registry_replaces_cache_when_provider_resource_changes");
        return;
    };
    let owner = format!("owner-{}", Uuid::new_v4());
    let computer = insert_computer_placeholder(&pool, &owner, "Sprite swap")
        .await
        .unwrap();
    let state = jwt_state(pool.clone());
    let registry = ComputerRegistry::default();

    let first = registry
        .connect_sprite(&state.config, &pool, &owner, &computer.id, true)
        .await
        .unwrap();

    sqlx::query(
        "UPDATE sandboxes SET provider_resource_id = $1 WHERE id = $2 AND owner_id = $3",
    )
    .bind(format!("elsewhere-swapped-{}", Uuid::new_v4()))
    .bind(&computer.id)
    .bind(&owner)
    .execute(&pool)
    .await
    .unwrap();

    let second = registry
        .connect_sprite(&state.config, &pool, &owner, &computer.id, true)
        .await
        .unwrap();

    assert!(!Arc::ptr_eq(&first, &second));
}
