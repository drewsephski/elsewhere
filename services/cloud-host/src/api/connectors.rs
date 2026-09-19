use axum::extract::State;
use axum::Extension;
use axum::Json;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::auth::Principal;
use std::sync::Arc;

use crate::connectors::{
    db::{
        consume_oauth_state, disconnect, get_for_owner, list_for_owner, load_github_credential,
        mark_reconnect_required, purge_expired_oauth_states, store_oauth_state,
        upsert_github_app_credential, ConnectorRow, GitHubCredentialLoad, PROVIDER_GITHUB,
    },
    github_access::parse_bot_chat_return_to,
    github_client::GitHubUser,
    service::collect_authorized_repositories,
    ConnectorSecretBox,
};
use crate::error::ApiError;
use crate::redact::redact_secrets;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorSummary {
    pub provider: String,
    pub status: String,
    pub metadata: serde_json::Value,
    pub connected_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<ConnectorRow> for ConnectorSummary {
    fn from(row: ConnectorRow) -> Self {
        Self {
            provider: row.provider,
            status: row.status,
            metadata: row.metadata,
            connected_at: row.connected_at,
            updated_at: row.updated_at,
        }
    }
}

fn secret_box(state: &AppState) -> Result<Arc<ConnectorSecretBox>, ApiError> {
    state
        .connector_secret_box()
        .ok_or_else(|| ApiError::Validation("connectors are not configured on this host".into()))
}

pub async fn list(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
) -> Result<Json<Vec<ConnectorSummary>>, ApiError> {
    let secret = state.connector_secret_box();
    let rows = list_for_owner(&state.pool, owner.owner_id()).await?;
    let mut summaries = Vec::new();
    for row in rows {
        summaries
            .push(github_aware_summary(&state, owner.owner_id(), secret.as_deref(), row).await?);
    }
    Ok(Json(summaries))
}

pub async fn github_status(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
) -> Result<Json<ConnectorSummary>, ApiError> {
    let row = get_for_owner(&state.pool, owner.owner_id(), PROVIDER_GITHUB).await?;
    Ok(Json(match row {
        Some(row) => {
            github_aware_summary(
                &state,
                owner.owner_id(),
                state.connector_secret_box().as_deref(),
                row,
            )
            .await?
        }
        None => ConnectorSummary {
            provider: PROVIDER_GITHUB.into(),
            status: "disconnected".into(),
            metadata: json!({}),
            connected_at: None,
            updated_at: Utc::now(),
        },
    }))
}

async fn github_aware_summary(
    state: &AppState,
    owner_id: &str,
    secret_box: Option<&ConnectorSecretBox>,
    row: ConnectorRow,
) -> Result<ConnectorSummary, ApiError> {
    if row.provider != PROVIDER_GITHUB {
        return Ok(ConnectorSummary::from(row));
    }
    let mut summary = ConnectorSummary::from(row);
    if summary.status != "connected" {
        return Ok(summary);
    }
    let Some(secret_box) = secret_box else {
        return Ok(summary);
    };
    match load_github_credential(&state.pool, owner_id, secret_box).await? {
        GitHubCredentialLoad::App(_) => Ok(summary),
        GitHubCredentialLoad::Missing => Ok(summary),
        GitHubCredentialLoad::ReconnectRequired => {
            summary.status = "reconnect_required".into();
            Ok(summary)
        }
        GitHubCredentialLoad::Legacy => {
            let _ = mark_reconnect_required(&state.pool, owner_id, PROVIDER_GITHUB).await?;
            summary.status = "reconnect_required".into();
            Ok(summary)
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GitHubOAuthStartRequest {
    pub return_to: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubOAuthStartResponse {
    pub authorize_url: String,
    pub state: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubOAuthCompleteResponse {
    pub provider: String,
    pub status: String,
    pub metadata: serde_json::Value,
    pub connected_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub return_to: Option<String>,
}

pub async fn github_oauth_start(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Json(body): Json<GitHubOAuthStartRequest>,
) -> Result<Json<GitHubOAuthStartResponse>, ApiError> {
    let _secret = secret_box(&state)?;
    let app_slug = state
        .config
        .github_app_slug
        .as_deref()
        .ok_or_else(|| ApiError::Validation("GitHub App is not configured".into()))?;
    let _client_id = state
        .config
        .github_client_id
        .as_deref()
        .ok_or_else(|| ApiError::Validation("GitHub App is not configured".into()))?;

    let return_to = match body
        .return_to
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(raw) => Some(
            parse_bot_chat_return_to(raw)
                .ok_or_else(|| ApiError::Validation("returnTo must be a bot chat path".into()))?,
        ),
        None => None,
    };

    purge_expired_oauth_states(&state.pool).await?;
    let state_token = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::minutes(10);
    store_oauth_state(
        &state.pool,
        &state_token,
        owner.owner_id(),
        PROVIDER_GITHUB,
        expires_at,
        return_to.as_deref(),
    )
    .await?;

    let authorize_url = state.github_client.installation_url(app_slug, &state_token);

    Ok(Json(GitHubOAuthStartResponse {
        authorize_url,
        state: state_token,
        expires_at,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubOAuthCompleteRequest {
    pub code: String,
    pub state: String,
    pub installation_id: Option<i64>,
}

pub async fn github_oauth_complete(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Json(body): Json<GitHubOAuthCompleteRequest>,
) -> Result<Json<GitHubOAuthCompleteResponse>, ApiError> {
    let secret_box = secret_box(&state)?;
    let client_id = state
        .config
        .github_client_id
        .as_deref()
        .ok_or_else(|| ApiError::Validation("GitHub App is not configured".into()))?;
    let client_secret = state
        .config
        .github_client_secret
        .as_deref()
        .ok_or_else(|| ApiError::Validation("GitHub App is not configured".into()))?;
    let redirect_uri = state
        .config
        .github_oauth_redirect_uri
        .as_deref()
        .ok_or_else(|| ApiError::Validation("GitHub App redirect URI is not configured".into()))?;

    let consumed =
        consume_oauth_state(&state.pool, &body.state, PROVIDER_GITHUB, owner.owner_id()).await?;
    let Some(consumed) = consumed else {
        return Err(ApiError::Validation(
            "invalid or expired OAuth state".into(),
        ));
    };

    let token = state
        .github_client
        .exchange_code(client_id, client_secret, &body.code, redirect_uri)
        .await
        .map_err(|e| ApiError::Validation(redact_secrets(&e)))?;
    let credential = token.into_credential(Utc::now());

    let user: GitHubUser = state
        .github_client
        .get_user(&credential.access_token)
        .await
        .map_err(|e| ApiError::Validation(redact_secrets(&e)))?;

    let installations = state
        .github_client
        .list_user_installations(&credential.access_token)
        .await
        .map_err(|e| ApiError::Validation(redact_secrets(&e)))?;
    if installations.is_empty() {
        return Err(ApiError::Validation(
            "GitHub App is not installed on any account accessible to this user".into(),
        ));
    }

    if let Some(claimed_id) = body.installation_id {
        if !installations.iter().any(|install| install.id == claimed_id) {
            tracing::info!(
                claimed_installation_id = claimed_id,
                "ignored GitHub installation id that is not accessible to the authorized user"
            );
        }
    }

    let repos = collect_authorized_repositories(
        &state.github_client,
        &credential.access_token,
        &installations,
    )
    .await
    .map_err(|e| ApiError::Validation(redact_secrets(&e.message())))?;

    let metadata = json!({
        "githubUser": {
            "login": user.login,
            "userId": user.id,
            "name": user.name,
            "avatarUrl": user.avatar_url,
        },
        "installations": installations.iter().map(|install| install.metadata_summary()).collect::<Vec<_>>(),
        "authorizedRepositoryCount": repos.len(),
    });

    let row = upsert_github_app_credential(
        &state.pool,
        owner.owner_id(),
        &metadata,
        &credential,
        secret_box.as_ref(),
    )
    .await?;
    state
        .connector_needs
        .notify_owner_github_changed(owner.owner_id())
        .await?;
    let summary = ConnectorSummary::from(row);
    Ok(Json(GitHubOAuthCompleteResponse {
        provider: summary.provider,
        status: summary.status,
        metadata: summary.metadata,
        connected_at: summary.connected_at,
        updated_at: summary.updated_at,
        return_to: consumed.return_to,
    }))
}

pub async fn github_disconnect(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
) -> Result<Json<ConnectorSummary>, ApiError> {
    disconnect(&state.pool, owner.owner_id(), PROVIDER_GITHUB).await?;
    state
        .connector_needs
        .notify_owner_github_changed(owner.owner_id())
        .await?;
    github_status(State(state), Extension(owner)).await
}
