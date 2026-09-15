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
        consume_oauth_state, disconnect, get_for_owner, list_for_owner, purge_expired_oauth_states,
        store_oauth_state, upsert_connected, ConnectorRow, PROVIDER_GITHUB,
    },
    github_client::GitHubUser,
    ConnectorSecretBox,
};
use crate::error::ApiError;

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
    let rows = list_for_owner(&state.pool, owner.owner_id()).await?;
    Ok(Json(rows.into_iter().map(ConnectorSummary::from).collect()))
}

pub async fn github_status(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
) -> Result<Json<ConnectorSummary>, ApiError> {
    let row = get_for_owner(&state.pool, owner.owner_id(), PROVIDER_GITHUB).await?;
    Ok(Json(match row {
        Some(row) => ConnectorSummary::from(row),
        None => ConnectorSummary {
            provider: PROVIDER_GITHUB.into(),
            status: "disconnected".into(),
            metadata: json!({}),
            connected_at: None,
            updated_at: Utc::now(),
        },
    }))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubOAuthStartResponse {
    pub authorize_url: String,
    pub state: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

pub async fn github_oauth_start(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
) -> Result<Json<GitHubOAuthStartResponse>, ApiError> {
    let _secret = secret_box(&state)?;
    let client_id = state
        .config
        .github_client_id
        .as_deref()
        .ok_or_else(|| ApiError::Validation("GitHub OAuth is not configured".into()))?;
    let redirect_uri = state
        .config
        .github_oauth_redirect_uri
        .as_deref()
        .ok_or_else(|| ApiError::Validation("GitHub OAuth redirect URI is not configured".into()))?;

    let _ = purge_expired_oauth_states(&state.pool).await?;
    let state_token = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::minutes(10);
    store_oauth_state(
        &state.pool,
        &state_token,
        owner.owner_id(),
        PROVIDER_GITHUB,
        expires_at,
    )
    .await?;

    let authorize_url = state
        .github_client
        .authorize_url(client_id, redirect_uri, &state_token);

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
}

pub async fn github_oauth_complete(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Json(body): Json<GitHubOAuthCompleteRequest>,
) -> Result<Json<ConnectorSummary>, ApiError> {
    let secret_box = secret_box(&state)?;
    let client_id = state
        .config
        .github_client_id
        .as_deref()
        .ok_or_else(|| ApiError::Validation("GitHub OAuth is not configured".into()))?;
    let client_secret = state
        .config
        .github_client_secret
        .as_deref()
        .ok_or_else(|| ApiError::Validation("GitHub OAuth is not configured".into()))?;
    let redirect_uri = state
        .config
        .github_oauth_redirect_uri
        .as_deref()
        .ok_or_else(|| ApiError::Validation("GitHub OAuth redirect URI is not configured".into()))?;

    let consumed = consume_oauth_state(
        &state.pool,
        &body.state,
        PROVIDER_GITHUB,
        owner.owner_id(),
    )
    .await?;
    if !consumed {
        return Err(ApiError::Validation("invalid or expired OAuth state".into()));
    }

    let token = state
        .github_client
        .exchange_code(client_id, client_secret, &body.code, redirect_uri)
        .await
        .map_err(|e| ApiError::Validation(crate::redact::redact_secrets(&e)))?;

    let user: GitHubUser = state
        .github_client
        .get_user(&token)
        .await
        .map_err(|e| ApiError::Validation(crate::redact::redact_secrets(&e)))?;

    let metadata = json!({
        "login": user.login,
        "userId": user.id,
        "name": user.name,
        "avatarUrl": user.avatar_url,
    });

    let row = upsert_connected(
        &state.pool,
        owner.owner_id(),
        PROVIDER_GITHUB,
        &metadata,
        &token,
        secret_box.as_ref(),
    )
    .await?;

    Ok(Json(ConnectorSummary::from(row)))
}

pub async fn github_disconnect(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
) -> Result<Json<ConnectorSummary>, ApiError> {
    disconnect(&state.pool, owner.owner_id(), PROVIDER_GITHUB).await?;
    github_status(State(state), Extension(owner)).await
}
