use axum::routing::{delete, get, post};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::api;
use crate::app_state::AppState;
use crate::auth::require_authenticated;

pub fn build_router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/v1/workspace", get(api::workspace::overview))
        .route("/v1/bots/{id}/context", get(api::context_results::get_context).put(api::context_results::save_context))
        .route("/v1/results", get(api::context_results::list_results))
        .route("/v1/results/{id}/download", get(api::context_results::download))
        .route("/v1/runs/{id}/results", get(api::context_results::run_results))
        .route(
            "/v1/routines",
            get(api::routines::list).post(api::routines::create),
        )
        .route(
            "/v1/routines/{id}",
            get(api::routines::get)
                .patch(api::routines::update)
                .put(api::routines::update)
                .delete(api::routines::delete),
        )
        .route(
            "/v1/routines/{id}/enabled",
            post(api::routines::set_enabled),
        )
        .route("/v1/routines/{id}/test", post(api::routines::test_run))
        .route("/v1/routines/{id}/run", post(api::routines::run_now))
        .route("/v1/routines/{id}/runs", get(api::routines::list_runs))
        .route("/v1/bots", get(api::bots::list).post(api::bots::create))
        .route(
            "/v1/bots/{id}",
            get(api::bots::get)
                .patch(api::bots::patch)
                .delete(api::bots::delete),
        )
        .route(
            "/v1/skills",
            get(api::skills::list).post(api::skills::create),
        )
        .route(
            "/v1/skills/{id}",
            get(api::skills::get)
                .patch(api::skills::patch)
                .delete(api::skills::delete),
        )
        .route(
            "/v1/skills/{id}/versions",
            get(api::skills::list_versions_handler).post(api::skills::create_version),
        )
        .route(
            "/v1/skills/{id}/versions/{version}",
            get(api::skills::get_version),
        )
        .route(
            "/v1/bots/{botId}/skills",
            get(api::skills::list_bot_skills_handler).post(api::skills::attach_bot_skill_handler),
        )
        .route(
            "/v1/bots/{botId}/skills/{skillId}",
            delete(api::skills::detach_bot_skill_handler),
        )
        .route(
            "/v1/runs/{id}/skill-draft",
            post(api::skills::create_run_skill_draft),
        )
        .route(
            "/v1/computers",
            get(api::computers::list).post(api::computers::create),
        )
        .route(
            "/v1/computers/{id}",
            get(api::computers::get).delete(api::computers::delete),
        )
        .route(
            "/v1/computers/{id}/browser-preview",
            get(api::computers::browser_preview),
        )
        .route(
            "/v1/computers/{id}/browser-control",
            get(api::computers::browser_control_state),
        )
        .route(
            "/v1/computers/{id}/browser-control/take",
            post(api::computers::browser_control_take),
        )
        .route(
            "/v1/computers/{id}/browser-control/return",
            post(api::computers::browser_control_return),
        )
        .route(
            "/v1/computers/{id}/browser-control/heartbeat",
            post(api::computers::browser_control_heartbeat),
        )
        .route(
            "/v1/computers/{id}/browser/navigate",
            post(api::computers::browser_navigate),
        )
        .route(
            "/v1/computers/{id}/browser/click",
            post(api::computers::browser_click),
        )
        .route(
            "/v1/computers/{id}/browser/type",
            post(api::computers::browser_type),
        )
        .route(
            "/v1/computers/{id}/browser/press-key",
            post(api::computers::browser_press_key),
        )
        .route(
            "/v1/computers/{id}/browser-profile/reset",
            post(api::computers::reset_browser_sign_in),
        )
        .route(
            "/v1/computers/{id}/workspace",
            get(api::computers::workspace_list),
        )
        .route(
            "/v1/computers/{id}/workspace-revision",
            get(api::computers::workspace_revision),
        )
        .route(
            "/v1/computers/{id}/workspace/file",
            get(api::computers::workspace_read)
                .delete(api::computers::workspace_delete)
                .patch(api::computers::workspace_rename),
        )
        .route("/v1/connectors", get(api::connectors::list))
        .route(
            "/v1/connectors/github",
            get(api::connectors::github_status).delete(api::connectors::github_disconnect),
        )
        .route(
            "/v1/connectors/github/oauth/start",
            post(api::connectors::github_oauth_start),
        )
        .route(
            "/v1/connectors/github/oauth/complete",
            post(api::connectors::github_oauth_complete),
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
        .route(
            "/v1/conversations",
            get(api::catalog::list_conversations).post(api::catalog::create_conversation),
        )
        .route(
            "/v1/conversations/groups",
            get(api::conversations::list_group_conversations)
                .post(api::conversations::create_group_conversation),
        )
        .route(
            "/v1/conversations/{id}",
            get(api::conversations::get_conversation)
                .patch(api::conversations::patch_conversation)
                .delete(api::conversations::delete_conversation),
        )
        .route(
            "/v1/conversations/{id}/messages",
            get(api::conversations::list_conversation_messages)
                .post(api::conversations::append_human_message_handler),
        )
        .route(
            "/v1/conversations/{id}/messages/{message_id}",
            delete(api::conversations::delete_conversation_message),
        )
        .route(
            "/v1/conversations/{id}/messages/{message_id}/route/retry",
            post(api::conversations::retry_message_route_handler),
        )
        .route(
            "/v1/conversations/{id}/participants",
            post(api::conversations::add_participant_handler),
        )
        .route(
            "/v1/conversations/{id}/participants/{bot_id}",
            delete(api::conversations::remove_participant_handler),
        )
        .route(
            "/v1/runs/{id}/delegations",
            get(api::delegations::list_run_delegations),
        )
        .route(
            "/v1/delegations/{id}",
            get(api::delegations::get_delegation_by_id),
        )
        .route("/v1/runs/{id}", get(api::runs::get_run))
        .route(
            "/v1/runs/{id}/human-intervention",
            get(crate::human_intervention::api::get_run_human_intervention),
        )
        .route("/v1/runs/{id}/cancel", post(api::runs::cancel_run))
        .route("/v1/runs/{id}/archive", post(api::runs::archive_run))
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
        .route("/ready", get(api::health::ready))
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
