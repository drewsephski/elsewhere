use axum::routing::{delete, get, patch, post};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::api;
use crate::app_state::AppState;
use crate::auth::require_authenticated;

pub fn build_router(state: AppState) -> Router {
    let protected = Router::new()
        .route(
            "/v1/routines",
            get(api::routines::list).post(api::routines::create),
        )
        .route(
            "/v1/routines/{id}",
            axum::routing::put(api::routines::update),
        )
        .route(
            "/v1/routines/{id}/enabled",
            post(api::routines::set_enabled),
        )
        .route("/v1/routines/{id}/run", post(api::routines::run_now))
        .route("/v1/bots", get(api::bots::list).post(api::bots::create))
        .route(
            "/v1/bots/{id}",
            get(api::bots::get)
                .patch(api::bots::patch)
                .delete(api::bots::delete),
        )
        .route(
            "/v1/computers",
            get(api::computers::list).post(api::computers::create),
        )
        .route(
            "/v1/computers/{id}",
            get(api::computers::get).delete(api::computers::delete),
        )
        .route("/v1/providers/status", get(api::providers::status))
        .route(
            "/v1/providers/codex/login/start",
            post(api::providers::codex_login_start),
        )
        .route(
            "/v1/providers/codex/login/status",
            get(api::providers::codex_login_status),
        )
        .route(
            "/v1/providers/codex/login/cancel",
            post(api::providers::codex_login_cancel),
        )
        .route(
            "/v1/runs",
            post(api::runs::create_run).get(api::catalog::list_runs),
        )
        .route("/v1/conversations", get(api::catalog::list_conversations))
        .route("/v1/runs/{id}", get(api::runs::get_run))
        .route("/v1/runs/{id}/cancel", post(api::runs::cancel_run))
        .route("/v1/runs/{id}/events", get(api::runs::run_events_sse))
        .route("/v1/approvals", get(crate::approval::api::list_approvals))
        .route(
            "/v1/approvals/{id}",
            get(crate::approval::api::get_approval),
        )
        .route(
            "/v1/approvals/{id}/approve",
            post(crate::approval::api::approve_approval),
        )
        .route(
            "/v1/approvals/{id}/deny",
            post(crate::approval::api::deny_approval),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_authenticated,
        ));

    let mut router = Router::new()
        .route("/health", get(api::health::health))
        .merge(protected)
        .with_state(state.clone())
        .layer(TraceLayer::new_for_http());

    if let Some(origin) = state.config.cors_web_origin.clone() {
        let cors = CorsLayer::new()
            .allow_origin(
                origin
                    .parse::<axum::http::HeaderValue>()
                    .unwrap_or_else(|_| {
                        tracing::warn!("invalid ELSEWHERE_WEB_ORIGIN; CORS not applied");
                        "http://127.0.0.1:0".parse().unwrap()
                    }),
            )
            .allow_methods(Any)
            .allow_headers(Any);
        router = router.layer(cors);
    }

    router
}
