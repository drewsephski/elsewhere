use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub service: &'static str,
}

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "elsewhere-cloud-host",
    })
}

/// Liveness is separate from readiness: the queue must have a recently healthy dispatcher.
pub fn runner_ready(state: &crate::AppState) -> bool {
    state
        .runner_heartbeat
        .lock()
        .ok()
        .and_then(|tick| *tick)
        .is_some_and(|tick| tick.elapsed() < std::time::Duration::from_secs(10))
}
pub async fn ready(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    let database = matches!(
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            sqlx::query("SELECT 1").execute(&state.pool)
        )
        .await,
        Ok(Ok(_))
    );
    let ready = database && runner_ready(&state);
    (
        if ready {
            axum::http::StatusCode::OK
        } else {
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        },
        Json(serde_json::json!({"ready": ready})),
    )
}
