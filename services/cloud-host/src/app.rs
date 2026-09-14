use axum::routing::{get, post};
use axum::Router;
use tower_http::trace::TraceLayer;

use crate::api;
use crate::app_state::AppState;
use crate::auth::require_bearer;

pub fn build_router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/v1/runs", post(api::runs::create_run))
        .route("/v1/runs/{id}", get(api::runs::get_run))
        .route("/v1/runs/{id}/cancel", post(api::runs::cancel_run))
        .route("/v1/runs/{id}/events", get(api::runs::run_events_sse))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_bearer,
        ));

    Router::new()
        .route("/health", get(api::health::health))
        .merge(protected)
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}
