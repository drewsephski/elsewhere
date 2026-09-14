use std::sync::Arc;

use axum::extract::State;
use axum::Extension;
use axum::Json;
use codex_provider::{
    probe_codex_subscription_availability, CodexAppServerClient, CodexProcessLaunch,
    CodexSubscriptionAvailability,
};
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::auth::Principal;
use crate::error::ApiError;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatusResponse {
    pub codex_installed: bool,
    pub chatgpt_connected: bool,
    pub chatgpt_plan_type: Option<String>,
    pub preferred_engine: String,
    pub api_fallback_configured: bool,
    pub default_model: String,
    pub codex_login_allowed: bool,
}

pub async fn status(
    State(state): State<AppState>,
    Extension(_principal): Extension<Principal>,
) -> Result<Json<ProviderStatusResponse>, ApiError> {
    let availability = probe_codex_subscription_availability(state.config.codex_executable.clone())
        .await;
    let (codex_installed, chatgpt_connected, plan_type) = map_availability(availability);

    Ok(Json(ProviderStatusResponse {
        codex_installed,
        chatgpt_connected,
        chatgpt_plan_type: plan_type,
        preferred_engine: format!("{:?}", state.config.run_engine).to_ascii_lowercase(),
        api_fallback_configured: state.config.openai_api_key.is_some(),
        default_model: agent_core::DEFAULT_MODEL.to_string(),
        codex_login_allowed: state.config.allow_codex_login,
    }))
}

fn map_availability(
    availability: CodexSubscriptionAvailability,
) -> (bool, bool, Option<String>) {
    match availability {
        CodexSubscriptionAvailability::Available { plan_type } => (true, true, plan_type),
        CodexSubscriptionAvailability::NotInstalled => (false, false, None),
        CodexSubscriptionAvailability::NotAuthenticated => (true, false, None),
        CodexSubscriptionAvailability::NotChatGpt => (true, false, None),
        CodexSubscriptionAvailability::Unavailable(_) => (true, false, None),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexLoginStartResponse {
    pub login_id: String,
    pub auth_url: String,
}

pub async fn codex_login_start(
    State(state): State<AppState>,
    Extension(_principal): Extension<Principal>,
) -> Result<Json<CodexLoginStartResponse>, ApiError> {
    if !state.config.allow_codex_login {
        return Err(ApiError::Validation(
            "Codex login is disabled on this deployment".into(),
        ));
    }
    let executable = state
        .config
        .codex_executable
        .clone()
        .ok_or_else(|| ApiError::Validation("Codex is not installed on this runner".into()))?;
    let launch = CodexProcessLaunch::from_path(executable).subscription_child();
    let client = CodexAppServerClient::launch(launch)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let handle = client
        .start_chatgpt_login()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    *state.codex_login_client.lock().await = Some(Arc::new(client));
    Ok(Json(CodexLoginStartResponse {
        login_id: handle.login_id,
        auth_url: handle.auth_url,
    }))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexLoginStatusResponse {
    pub connected: bool,
    pub plan_type: Option<String>,
}

pub async fn codex_login_status(
    State(state): State<AppState>,
    Extension(_principal): Extension<Principal>,
) -> Result<Json<CodexLoginStatusResponse>, ApiError> {
    let availability = probe_codex_subscription_availability(state.config.codex_executable.clone())
        .await;
    let (_, connected, plan) = map_availability(availability);
    Ok(Json(CodexLoginStatusResponse {
        connected,
        plan_type: plan,
    }))
}

#[derive(Debug, Deserialize)]
pub struct CodexLoginCancelRequest {
    #[serde(rename = "loginId")]
    pub login_id: String,
}

pub async fn codex_login_cancel(
    State(state): State<AppState>,
    Extension(_principal): Extension<Principal>,
    Json(body): Json<CodexLoginCancelRequest>,
) -> Result<axum::http::StatusCode, ApiError> {
    if !state.config.allow_codex_login {
        return Err(ApiError::Validation(
            "Codex login is disabled on this deployment".into(),
        ));
    }
    if let Some(client) = state.codex_login_client.lock().await.take() {
        let _ = client.cancel_login(&body.login_id).await;
        if let Ok(client) = Arc::try_unwrap(client) {
            let _ = client.shutdown().await;
        }
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}
