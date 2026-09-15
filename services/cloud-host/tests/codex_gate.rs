//! Codex gate integration: HTTP liveness must not block on held Codex permits.

mod support;

use std::sync::atomic::Ordering;
use std::time::Duration;

use cloud_host::codex_ops::CodexOperationKind;
use cloud_host::{build_router, AppState};
use sqlx::PgPool;
use support::test_config;
use tower::ServiceExt;

#[sqlx::test(migrations = "./migrations")]
async fn health_stays_responsive_during_slow_codex_operation(pool: PgPool) {
    let state = AppState::new(pool, test_config());
    state.dispatcher_alive.store(true, Ordering::SeqCst);

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
        let response = tokio::time::timeout(
            Duration::from_millis(500),
            build_router(state.clone()).oneshot(
                http::Request::builder()
                    .uri("/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            ),
        )
        .await
        .expect("health request timed out")
        .expect("health request failed");
        assert_eq!(response.status(), http::StatusCode::OK);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    hold.await.expect("slow codex simulation");
}
