use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::Extension;
use axum::Json;
use codex_provider::{
    probe_codex_subscription_availability_with_profile, CodexAppServerClient, CodexProcessLaunch,
    CodexSubscriptionAvailability,
};
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::auth::Principal;
use crate::error::ApiError;

/// Bound how long status checks block on Codex app-server probes (launch + account read).
const PROVIDER_STATUS_PROBE_TIMEOUT: Duration = Duration::from_secs(30);

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
    Extension(principal): Extension<Principal>,
) -> Result<Json<ProviderStatusResponse>, ApiError> {
    let availability =
        owner_availability_with_timeout(&state, principal.owner_id()).await?;
    let (codex_installed, chatgpt_connected, plan_type) = map_availability(availability);

    Ok(Json(ProviderStatusResponse {
        codex_installed,
        chatgpt_connected,
        chatgpt_plan_type: plan_type,
        preferred_engine: format!("{:?}", state.config.run_engine).to_ascii_lowercase(),
        api_fallback_configured: state.config.openai_api_key.is_some(),
        default_model: agent_core::DEFAULT_MODEL.to_string(),
        codex_login_allowed: state.config.allow_codex_login
            && (principal.owner_id() == crate::auth::LEGACY_LOCAL_OWNER
                || state.config.codex_profiles_dir.is_some()),
    }))
}

fn map_availability(availability: CodexSubscriptionAvailability) -> (bool, bool, Option<String>) {
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
    pub user_code: String,
}

pub async fn codex_login_start(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
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
    let profile = crate::provider_profile::profile_for_owner(
        &state.pool,
        &state.config,
        principal.owner_id(),
    )
    .await?;
    // Bound sign-in process concurrency; another user cannot observe or replace it.
    let mut pending = state.codex_login_client.lock().await;
    if pending
        .as_ref()
        .is_some_and(|login| login.expires_at <= std::time::Instant::now())
    {
        pending.take();
    }
    if let Some(login) = pending.as_ref() {
        if login.owner_id == principal.owner_id() {
            return Ok(Json(CodexLoginStartResponse {
                login_id: login.login_id.clone(),
                auth_url: login.auth_url.clone(),
                user_code: login.user_code.clone(),
            }));
        }
        return Err(ApiError::Conflict(
            "ChatGPT sign-in is busy. Please try again shortly.".into(),
        ));
    }
    let mut launch = CodexProcessLaunch::from_path(executable).subscription_child();
    if let Some(profile) = profile {
        launch = launch.with_profile(&profile);
    }
    let client = CodexAppServerClient::launch(launch)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let handle = client
        .start_chatgpt_device_login()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    *pending = Some(crate::app_state::PendingCodexLogin {
        owner_id: principal.owner_id().to_string(),
        login_id: handle.login_id.clone(),
        auth_url: handle.verification_url.clone(),
        user_code: handle.user_code.clone(),
        expires_at: std::time::Instant::now() + std::time::Duration::from_secs(600),
        client: Arc::new(client),
    });
    let pending_logins = state.codex_login_client.clone();
    let expiring_id = handle.login_id.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(600)).await;
        let mut pending = pending_logins.lock().await;
        if pending
            .as_ref()
            .is_some_and(|login| login.login_id == expiring_id)
        {
            pending.take();
        }
    });
    Ok(Json(CodexLoginStartResponse {
        login_id: handle.login_id,
        auth_url: handle.verification_url,
        user_code: handle.user_code,
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
    Extension(principal): Extension<Principal>,
) -> Result<Json<CodexLoginStatusResponse>, ApiError> {
    let availability = owner_availability(&state, principal.owner_id()).await?;
    let (_, connected, plan) = map_availability(availability);
    if connected {
        let mut pending = state.codex_login_client.lock().await;
        if pending
            .as_ref()
            .is_some_and(|login| login.owner_id == principal.owner_id())
        {
            pending.take();
        }
    }
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
    Extension(principal): Extension<Principal>,
    Json(body): Json<CodexLoginCancelRequest>,
) -> Result<axum::http::StatusCode, ApiError> {
    if !state.config.allow_codex_login {
        return Err(ApiError::Validation(
            "Codex login is disabled on this deployment".into(),
        ));
    }
    let mut pending = state.codex_login_client.lock().await;
    let login = pending
        .as_ref()
        .filter(|login| login.owner_id == principal.owner_id() && login.login_id == body.login_id)
        .ok_or(ApiError::NotFound)?;
    login
        .client
        .cancel_login(&body.login_id)
        .await
        .map_err(|_| ApiError::Internal("Could not cancel ChatGPT sign-in".into()))?;
    pending.take();
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn owner_availability(
    state: &AppState,
    owner_id: &str,
) -> Result<CodexSubscriptionAvailability, ApiError> {
    if owner_id != crate::auth::LEGACY_LOCAL_OWNER && state.config.codex_profiles_dir.is_none() {
        return Ok(CodexSubscriptionAvailability::NotAuthenticated);
    }
    let profile =
        crate::provider_profile::profile_for_owner(&state.pool, &state.config, owner_id).await?;
    Ok(probe_codex_subscription_availability_with_profile(
        state.config.codex_executable.clone(),
        profile,
    )
    .await)
}

async fn owner_availability_with_timeout(
    state: &AppState,
    owner_id: &str,
) -> Result<CodexSubscriptionAvailability, ApiError> {
    match tokio::time::timeout(
        PROVIDER_STATUS_PROBE_TIMEOUT,
        owner_availability(state, owner_id),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Ok(CodexSubscriptionAvailability::Unavailable(
            "ChatGPT connection check timed out on the runner".into(),
        )),
    }
}
