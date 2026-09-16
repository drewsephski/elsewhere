use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::app_state::AppState;
use crate::auth::Principal;
use crate::channels::db::{
    disconnect, find_connected_slack_workspace, get_for_owner, insert_received_event,
    list_for_owner, load_access_token, set_default_bot, upsert_slack_connection,
    ChannelConnectionRow,
};
use crate::channels::oauth::{self, random_oauth_state};
use crate::channels::slack::events::{normalize_slack_event, url_verification_challenge};
use crate::channels::slack::signature::verify_slack_request;
use crate::channels::types::NormalizedInbound;
use crate::channels::{MAX_EVENT_BODY_BYTES, PROVIDER_SLACK};
use crate::connectors::ConnectorSecretBox;
use crate::db::resources::get_bot_for_owner;
use crate::error::ApiError;
use std::sync::Arc;

fn secret_box(state: &AppState) -> Result<Arc<ConnectorSecretBox>, ApiError> {
    state.connector_secret_box().ok_or_else(|| {
        ApiError::Validation("channel secrets are not configured on this host".into())
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelConnectionSummary {
    pub id: String,
    pub provider: String,
    pub status: String,
    pub enabled: bool,
    pub workspace_name: Option<String>,
    pub default_bot_id: Option<String>,
    pub default_bot_name: Option<String>,
    pub connected_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl ChannelConnectionSummary {
    async fn from_row(state: &AppState, row: ChannelConnectionRow) -> Result<Self, ApiError> {
        let bot_name = if let Some(bot_id) = row.default_bot_id.as_deref() {
            sqlx::query_scalar::<_, String>("SELECT name FROM bots WHERE id = $1 AND owner_id = $2")
                .bind(bot_id)
                .bind(&row.owner_id)
                .fetch_optional(&state.pool)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?
        } else {
            None
        };
        Ok(Self {
            id: row.id,
            provider: row.provider,
            status: row.status,
            enabled: row.enabled,
            workspace_name: row.workspace_name,
            default_bot_id: row.default_bot_id,
            default_bot_name: bot_name,
            connected_at: Some(row.created_at),
            updated_at: row.updated_at,
        })
    }
}

pub async fn list_channels(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
) -> Result<Json<Vec<ChannelConnectionSummary>>, ApiError> {
    let rows = list_for_owner(&state.pool, owner.owner_id()).await?;
    let mut out = Vec::new();
    for row in rows {
        out.push(ChannelConnectionSummary::from_row(&state, row).await?);
    }
    Ok(Json(out))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlackOAuthStartRequest {
    pub bot_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlackOAuthStartResponse {
    pub authorize_url: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

pub async fn slack_oauth_start(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Json(body): Json<SlackOAuthStartRequest>,
) -> Result<Json<SlackOAuthStartResponse>, ApiError> {
    let _ = secret_box(&state)?;
    let client_id = state
        .config
        .slack_client_id
        .as_deref()
        .ok_or_else(|| ApiError::Validation("Slack OAuth is not configured".into()))?;
    let redirect_uri = state
        .config
        .slack_oauth_redirect_uri
        .as_deref()
        .ok_or_else(|| ApiError::Validation("Slack OAuth redirect URI is not configured".into()))?;
    let bot = get_bot_for_owner(&state.pool, owner.owner_id(), &body.bot_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::Validation("Choose a Bot to talk to from Slack".into()))?;

    let _ = oauth::purge_expired(&state.pool).await?;
    let state_token = random_oauth_state();
    let expires_at = oauth::default_expiry();
    oauth::store(
        &state.pool,
        &state_token,
        owner.owner_id(),
        PROVIDER_SLACK,
        &bot.id,
        expires_at,
    )
    .await?;

    let authorize_url = state
        .slack_client
        .authorize_url(client_id, redirect_uri, &state_token);
    Ok(Json(SlackOAuthStartResponse {
        authorize_url,
        expires_at,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlackOAuthCompleteRequest {
    pub code: String,
    pub state: String,
}

pub async fn slack_oauth_complete(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Json(body): Json<SlackOAuthCompleteRequest>,
) -> Result<Json<ChannelConnectionSummary>, ApiError> {
    let secret = secret_box(&state)?;
    let client_id = state
        .config
        .slack_client_id
        .as_deref()
        .ok_or_else(|| ApiError::Validation("Slack OAuth is not configured".into()))?;
    let client_secret = state
        .config
        .slack_client_secret
        .as_deref()
        .ok_or_else(|| ApiError::Validation("Slack OAuth is not configured".into()))?;
    let redirect_uri = state
        .config
        .slack_oauth_redirect_uri
        .as_deref()
        .ok_or_else(|| ApiError::Validation("Slack OAuth redirect URI is not configured".into()))?;

    let consumed =
        oauth::consume(&state.pool, &body.state, PROVIDER_SLACK, owner.owner_id()).await?;
    let Some(consumed) = consumed else {
        return Err(ApiError::Validation(
            "invalid or expired OAuth state".into(),
        ));
    };
    let bot_id = consumed
        .bot_id
        .ok_or_else(|| ApiError::Validation("OAuth state is missing a Bot".into()))?;

    let install = state
        .slack_client
        .exchange_code(client_id, client_secret, &body.code, redirect_uri)
        .await
        .map_err(|e| ApiError::Validation(crate::redact::redact_secrets(&e)))?;

    let row = upsert_slack_connection(
        &state.pool,
        owner.owner_id(),
        &install.workspace_id,
        install.workspace_name.as_deref(),
        &install.installer_user_id,
        install.bot_user_id.as_deref(),
        &bot_id,
        &install.access_token,
        secret.as_ref(),
    )
    .await?;
    Ok(Json(ChannelConnectionSummary::from_row(&state, row).await?))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchChannelRequest {
    pub default_bot_id: String,
}

pub async fn patch_channel(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<String>,
    Json(body): Json<PatchChannelRequest>,
) -> Result<Json<ChannelConnectionSummary>, ApiError> {
    get_bot_for_owner(&state.pool, owner.owner_id(), &body.default_bot_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::Validation("Choose a Bot to talk to from Slack".into()))?;
    let row = set_default_bot(&state.pool, owner.owner_id(), &id, &body.default_bot_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(ChannelConnectionSummary::from_row(&state, row).await?))
}

pub async fn disconnect_channel(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<String>,
) -> Result<Json<ChannelConnectionSummary>, ApiError> {
    let existing = get_for_owner(&state.pool, owner.owner_id(), &id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if existing.status == "connected" {
        if let (Some(secret), Some(client_id), Some(client_secret)) = (
            state.connector_secret_box(),
            state.config.slack_client_id.as_deref(),
            state.config.slack_client_secret.as_deref(),
        ) {
            if let Ok(Some(token)) = load_access_token(&state.pool, &id, secret.as_ref()).await {
                if let Err(err) = state
                    .slack_client
                    .uninstall(&token, client_id, client_secret)
                    .await
                {
                    tracing::info!(
                        error = %crate::redact::redact_secrets(&err),
                        "Slack token revocation skipped; local disconnect will proceed"
                    );
                }
            }
        }
    }
    disconnect(&state.pool, owner.owner_id(), &id).await?;
    let row = get_for_owner(&state.pool, owner.owner_id(), &id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(ChannelConnectionSummary::from_row(&state, row).await?))
}

pub async fn slack_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    if body.len() > MAX_EVENT_BODY_BYTES {
        return ApiError::PayloadTooLarge.into_response();
    }
    let Some(signing_secret) = state.config.slack_signing_secret.as_deref() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "Slack is not configured"})),
        )
            .into_response();
    };
    let timestamp = headers
        .get("x-slack-request-timestamp")
        .and_then(|v| v.to_str().ok());
    let signature = headers
        .get("x-slack-signature")
        .and_then(|v| v.to_str().ok());
    let now = chrono::Utc::now().timestamp();
    if let Err(err) = verify_slack_request(signing_secret, timestamp, signature, &body, now) {
        tracing::info!(reason = err.as_str(), "rejected Slack Events API request");
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "unauthorized"})),
        )
            .into_response();
    }

    let parsed: Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "invalid json"})),
            )
                .into_response();
        }
    };
    if let Some(challenge) = url_verification_challenge(&parsed) {
        return (StatusCode::OK, Json(json!({"challenge": challenge}))).into_response();
    }

    let inbound = normalize_slack_event(&parsed);
    let event_id = parsed.get("event_id").and_then(Value::as_str).unwrap_or("");
    let team_id = parsed.get("team_id").and_then(Value::as_str).unwrap_or("");
    if event_id.is_empty() || team_id.is_empty() {
        return (StatusCode::OK, Json(json!({"ok": true}))).into_response();
    }
    let idempotency_key = format!("slack:{team_id}:{event_id}");
    let connection = find_connected_slack_workspace(&state.pool, team_id)
        .await
        .ok()
        .flatten();
    let payload = inbound
        .as_ref()
        .map(NormalizedInbound::to_payload)
        .unwrap_or_else(|| json!({}));
    let inserted = match insert_received_event(
        &state.pool,
        PROVIDER_SLACK,
        event_id,
        &idempotency_key,
        inbound.as_ref().map(|v| v.event_type.as_str()),
        connection.as_ref().map(|c| c.id.as_str()),
        connection.as_ref().map(|c| c.owner_id.as_str()),
        &payload,
    )
    .await
    {
        Ok(id) => id,
        Err(err) => {
            tracing::warn!(
                error = %crate::redact::redact_secrets(&err.to_string()),
                "failed to persist Slack event"
            );
            return ApiError::Internal("could not persist event".into()).into_response();
        }
    };

    if inserted.is_none() {
        return (StatusCode::OK, Json(json!({"ok": true}))).into_response();
    }
    let event_row_id = inserted.unwrap();
    match inbound {
        Some(inbound) => {
            if let Err(err) =
                crate::channels::admission::admit_normalized(&state, &event_row_id, &inbound).await
            {
                tracing::warn!(
                    error = %crate::redact::redact_secrets(&err.to_string()),
                    "channel event admission failed"
                );
            }
        }
        None => {
            let _ = crate::channels::db::mark_event_status(
                &state.pool,
                &event_row_id,
                "ignored",
                Some("unsupported_event"),
                None,
            )
            .await;
        }
    }

    (StatusCode::OK, Json(json!({"ok": true}))).into_response()
}
