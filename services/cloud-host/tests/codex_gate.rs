use std::time::Duration;

use cloud_host::codex_ops::{CodexOperationKind, CodexOpsGate};
use cloud_host::{build_router, AppState, Config};
use sqlx::PgPool;
use tower::ServiceExt;

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
        openai_api_key: Some("test-key".into()),
        sprite_token: "test-sprite".into(),
        api_token: "test-token".into(),
        auth_mode: cloud_host::config::AuthMode::InternalToken,
        jwt_issuer: None,
        jwt_audience: None,
        jwt_jwks_url: None,
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

#[tokio::test]
async fn codex_gate_allows_only_one_active_operation() {
    let gate = CodexOpsGate::from_permits(1);
    let run = gate.try_acquire(CodexOperationKind::Run).expect("run permit");
    assert_eq!(gate.active_children(), 1);
    assert!(gate.try_acquire(CodexOperationKind::Probe).is_err());
    assert!(gate.try_acquire(CodexOperationKind::Login).is_err());
    drop(run);
    assert_eq!(gate.active_children(), 0);
    assert!(gate.try_acquire(CodexOperationKind::Probe).is_ok());
}

#[tokio::test]
async fn provider_status_and_run_do_not_overlap() {
    let gate = CodexOpsGate::from_permits(1);
    let run = gate.try_acquire(CodexOperationKind::Run).expect("run");
    assert!(gate.try_acquire(CodexOperationKind::Probe).is_err());
    drop(run);
    let probe = gate.try_acquire(CodexOperationKind::Probe).expect("probe");
    assert!(gate.try_acquire(CodexOperationKind::Run).is_err());
    drop(probe);
}

#[tokio::test]
async fn auto_probe_and_run_share_one_slot() {
    let gate = CodexOpsGate::from_permits(1);
    let probe = gate.try_acquire(CodexOperationKind::Probe).expect("probe");
    assert!(gate.try_acquire(CodexOperationKind::Run).is_err());
    drop(probe);
    let run = gate.try_acquire(CodexOperationKind::Run).expect("run");
    assert!(gate.try_acquire(CodexOperationKind::Probe).is_err());
    drop(run);
}

#[tokio::test]
async fn device_login_blocks_provider_status_probe() {
    let gate = CodexOpsGate::from_permits(1);
    let login = gate.try_acquire(CodexOperationKind::Login).expect("login");
    assert!(gate.try_acquire(CodexOperationKind::Probe).is_err());
    drop(login);
}

#[tokio::test]
async fn device_login_blocks_run_admission() {
    let gate = CodexOpsGate::from_permits(1);
    let login = gate.try_acquire(CodexOperationKind::Login).expect("login");
    assert!(gate.try_acquire(CodexOperationKind::Run).is_err());
    drop(login);
}

#[tokio::test]
async fn group_route_waits_behind_run_permit() {
    let gate = CodexOpsGate::from_permits(1);
    let run = gate.try_acquire(CodexOperationKind::Run).expect("run");
    assert!(gate.try_acquire(CodexOperationKind::GroupRoute).is_err());
    drop(run);
    assert!(gate.try_acquire(CodexOperationKind::GroupRoute).is_ok());
}

#[tokio::test]
async fn health_stays_responsive_during_slow_codex_operation() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping health_stays_responsive_during_slow_codex_operation: no database");
        return;
    };
    let state = AppState::new(pool, test_config());
    state
        .dispatcher_alive
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let permit = state
        .codex_ops
        .acquire(CodexOperationKind::Run)
        .await
        .expect("codex permit");
    let hold = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(250)).await;
        drop(permit);
    });

    for _ in 0..12 {
        let response = build_router(state.clone())
            .oneshot(
                http::Request::builder()
                    .uri("/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .expect("health request");
        assert_eq!(response.status(), http::StatusCode::OK);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    hold.await.expect("slow codex simulation");
}
