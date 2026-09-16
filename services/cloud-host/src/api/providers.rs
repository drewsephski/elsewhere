use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::Extension;
use axum::Json;
use codex_provider::{
    probe_codex_subscription_availability_on_client, CodexAppServerClient, CodexProcessLaunch,
    CodexSubscriptionAvailability,
};
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::auth::Principal;
use crate::codex_ops::{CodexOperationKind, CODEX_BUSY_REASON};
use crate::error::ApiError;

/// Background refresh only; `/v1/providers/status` must not block on Codex probes.
const PROVIDER_STATUS_BACKGROUND_PROBE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatusResponse {
    pub codex_installed: bool,
    pub chatgpt_connected: bool,
    /// connected | not_connected | unavailable
    pub chatgpt_connection_state: &'static str,
    /// Stable non-secret reason code for UI/diagnostics.
    pub connection_detail: Option<&'static str>,
    pub chatgpt_plan_type: Option<String>,
    pub preferred_engine: String,
    pub api_fallback_configured: bool,
    pub default_model: String,
    pub codex_login_allowed: bool,
}

#[derive(Debug)]
struct AvailabilityView {
    codex_installed: bool,
    connected: bool,
    plan_type: Option<String>,
    connection_state: &'static str,
    detail: Option<&'static str>,
}

pub async fn status(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
) -> Result<Json<ProviderStatusResponse>, ApiError> {
    let availability = owner_availability_for_status(&state, principal.owner_id()).await?;
    let view = map_availability(availability);

    Ok(Json(ProviderStatusResponse {
        codex_installed: view.codex_installed,
        chatgpt_connected: view.connected,
        chatgpt_connection_state: view.connection_state,
        connection_detail: view.detail,
        chatgpt_plan_type: view.plan_type,
        preferred_engine: format!("{:?}", state.config.run_engine).to_ascii_lowercase(),
        api_fallback_configured: state.config.openai_api_key.is_some(),
        default_model: agent_core::DEFAULT_MODEL.to_string(),
        codex_login_allowed: state.config.allow_codex_login
            && (principal.owner_id() == crate::auth::LEGACY_LOCAL_OWNER
                || state.config.codex_profiles_dir.is_some()),
    }))
}

fn map_availability(availability: CodexSubscriptionAvailability) -> AvailabilityView {
    match availability {
        CodexSubscriptionAvailability::Available { plan_type } => AvailabilityView {
            codex_installed: true,
            connected: true,
            plan_type,
            connection_state: "connected",
            detail: None,
        },
        CodexSubscriptionAvailability::NotInstalled => AvailabilityView {
            codex_installed: false,
            connected: false,
            plan_type: None,
            connection_state: "unavailable",
            detail: Some("codex_not_installed"),
        },
        CodexSubscriptionAvailability::NotAuthenticated => AvailabilityView {
            codex_installed: true,
            connected: false,
            plan_type: None,
            connection_state: "not_connected",
            detail: Some("not_authenticated"),
        },
        CodexSubscriptionAvailability::NotChatGpt => AvailabilityView {
            codex_installed: true,
            connected: false,
            plan_type: None,
            connection_state: "not_connected",
            detail: Some("not_chatgpt"),
        },
        CodexSubscriptionAvailability::Unavailable(reason) => {
            let detail = if reason == CODEX_BUSY_REASON {
                "codex_busy"
            } else {
                "codex_probe_unavailable"
            };
            AvailabilityView {
                codex_installed: true,
                connected: false,
                plan_type: None,
                connection_state: "unavailable",
                detail: Some(detail),
            }
        }
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
    let codex_permit = state
        .codex_ops
        .try_acquire(CodexOperationKind::Login)
        .map_err(|_| ApiError::Conflict("Codex is busy. Please try again shortly.".into()))?;
    let mut launch = CodexProcessLaunch::from_path(executable).subscription_child();
    if let Some(profile) = profile {
        launch = launch.with_profile(&profile);
    }
    let client = CodexAppServerClient::launch(launch)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    codex_permit.log_child_started();
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
        codex_permit,
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
    let pending_client = {
        let pending = state.codex_login_client.lock().await;
        pending
            .as_ref()
            .filter(|login| login.owner_id == principal.owner_id())
            .map(|login| login.client.clone())
    };

    let availability = if let Some(client) = pending_client {
        probe_codex_subscription_availability_on_client(&client).await
    } else {
        owner_availability(&state, principal.owner_id()).await?
    };
    let view = map_availability(availability.clone());
    if view.connected {
        state
            .provider_status_cache
            .store(principal.owner_id(), availability);
        let mut pending = state.codex_login_client.lock().await;
        if pending
            .as_ref()
            .is_some_and(|login| login.owner_id == principal.owner_id())
        {
            pending.take();
        }
    }
    Ok(Json(CodexLoginStatusResponse {
        connected: view.connected,
        plan_type: view.plan_type,
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

async fn owner_availability_for_status(
    state: &AppState,
    owner_id: &str,
) -> Result<CodexSubscriptionAvailability, ApiError> {
    if state
        .codex_login_client
        .lock()
        .await
        .as_ref()
        .is_some_and(|login| login.owner_id == owner_id)
    {
        return Ok(CodexSubscriptionAvailability::NotAuthenticated);
    }

    if state.provider_status_cache.is_fresh(owner_id) {
        return Ok(state.provider_status_cache.get(owner_id).unwrap_or(
            CodexSubscriptionAvailability::Unavailable("Provider status cache missing".into()),
        ));
    }

    if let Some(cached) = state.provider_status_cache.get(owner_id) {
        schedule_provider_status_refresh(state.clone(), owner_id.to_string());
        return Ok(cached);
    }

    schedule_provider_status_refresh(state.clone(), owner_id.to_string());
    Ok(CodexSubscriptionAvailability::Unavailable(
        "Provider status is refreshing".into(),
    ))
}

fn schedule_provider_status_refresh(state: AppState, owner_id: String) {
    if state.provider_status_cache.is_fresh(&owner_id) {
        return;
    }
    if !state
        .provider_status_cache
        .try_begin_background_refresh(&owner_id)
    {
        return;
    }
    tokio::spawn(async move {
        let availability = match tokio::time::timeout(
            PROVIDER_STATUS_BACKGROUND_PROBE_TIMEOUT,
            owner_availability_for_status_probe(&state, &owner_id),
        )
        .await
        {
            Ok(Ok(availability)) => availability,
            Ok(Err(err)) => {
                tracing::warn!(owner_id = %owner_id, error = %err, "provider status refresh failed");
                CodexSubscriptionAvailability::Unavailable(err.to_string())
            }
            Err(_) => CodexSubscriptionAvailability::Unavailable(
                "Codex availability check timed out".into(),
            ),
        };
        state.provider_status_cache.store(&owner_id, availability);
        state
            .provider_status_cache
            .end_background_refresh(&owner_id);
    });
}

async fn owner_availability_for_status_probe(
    state: &AppState,
    owner_id: &str,
) -> Result<CodexSubscriptionAvailability, ApiError> {
    if owner_id != crate::auth::LEGACY_LOCAL_OWNER && state.config.codex_profiles_dir.is_none() {
        return Ok(CodexSubscriptionAvailability::NotAuthenticated);
    }
    let profile =
        crate::provider_profile::profile_for_owner(&state.pool, &state.config, owner_id).await?;
    let permit = match state.codex_ops.try_acquire(CodexOperationKind::Probe) {
        Ok(permit) => permit,
        Err(()) => {
            return Ok(CodexSubscriptionAvailability::Unavailable(
                CODEX_BUSY_REASON.into(),
            ))
        }
    };
    Ok(crate::codex_ops::probe_subscription_with_profile(
        state.config.codex_executable.clone(),
        profile,
        &permit,
    )
    .await)
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
    let permit = state
        .codex_ops
        .acquire(CodexOperationKind::Probe)
        .await
        .map_err(|_| ApiError::Internal("Codex is busy on this host".into()))?;
    Ok(crate::codex_ops::probe_subscription_with_profile(
        state.config.codex_executable.clone(),
        profile,
        &permit,
    )
    .await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_probe_is_not_reported_as_disconnected() {
        let view = map_availability(CodexSubscriptionAvailability::Unavailable("timeout".into()));
        assert!(!view.connected);
        assert_eq!(view.connection_state, "unavailable");
        assert_eq!(view.detail, Some("codex_probe_unavailable"));
    }

    #[test]
    fn codex_busy_is_reported_without_spawning() {
        let view = map_availability(CodexSubscriptionAvailability::Unavailable(
            CODEX_BUSY_REASON.into(),
        ));
        assert!(!view.connected);
        assert_eq!(view.connection_state, "unavailable");
        assert_eq!(view.detail, Some("codex_busy"));
    }

    #[test]
    fn unauthenticated_profile_is_explicitly_not_connected() {
        let view = map_availability(CodexSubscriptionAvailability::NotAuthenticated);
        assert!(!view.connected);
        assert_eq!(view.connection_state, "not_connected");
        assert_eq!(view.detail, Some("not_authenticated"));
    }
}
