use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::auth::{require_jwt, Principal};
use crate::error::ApiError;
use crate::local_mac::db::{self, ExchangeFailure};
use crate::local_mac::{
    is_plausible_installation_id, is_plausible_user_code, max_pending_per_installation,
    normalize_device_name, require_credential_key,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePairingRequest {
    pub device_name: String,
    pub installation_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePairingResponse {
    pub pairing_id: String,
    pub pairing_secret: String,
    pub user_code: String,
    pub expires_at: DateTime<Utc>,
    pub verification_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingView {
    pub pairing_id: String,
    pub device_name: String,
    pub expires_at: DateTime<Utc>,
    pub status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovePairingRequest {
    pub user_code: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovePairingResponse {
    pub pairing_id: String,
    pub device_name: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangePairingRequest {
    pub pairing_secret: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangePairingResponse {
    pub node_id: String,
    pub computer_id: String,
    pub credential: String,
    pub display_name: String,
}

#[derive(Debug, Serialize)]
struct StatusBody {
    status: String,
}

pub async fn create_pairing(
    State(state): State<AppState>,
    Json(body): Json<CreatePairingRequest>,
) -> Result<Json<CreatePairingResponse>, ApiError> {
    let key = require_credential_key(state.config.local_mac_credential_key.as_ref())?;
    let device_name = normalize_device_name(&body.device_name)?;
    let installation_id = body.installation_id.trim();
    if !is_plausible_installation_id(installation_id) {
        return Err(ApiError::Validation("installationId must be a UUID".into()));
    }
    state.local_mac_pairing_limiter.check(installation_id)?;
    let pending = db::count_pending_for_installation(&state.pool, installation_id).await?;
    if pending >= max_pending_per_installation() {
        return Err(ApiError::RateLimited(
            "too many pending pairing attempts from this Mac".into(),
        ));
    }
    let created = db::insert_pairing_session(
        &state.pool,
        key,
        installation_id,
        &device_name,
        state.config.cors_web_origin.as_deref(),
    )
    .await?;
    Ok(Json(CreatePairingResponse {
        pairing_id: created.pairing_id,
        pairing_secret: created.pairing_secret,
        user_code: created.user_code,
        expires_at: created.expires_at,
        verification_url: created.verification_url,
    }))
}

pub async fn get_pairing(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(pairing_id): Path<String>,
) -> Result<Json<PairingView>, ApiError> {
    require_jwt(&principal)?;
    let row = db::get_pairing_session(&state.pool, &pairing_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if row.consumed_at.is_some() {
        return Err(ApiError::NotFound);
    }
    if let Some(owner_id) = row.owner_id.as_deref() {
        if owner_id != principal.owner_id() {
            return Err(ApiError::NotFound);
        }
    }
    let status = if row.expires_at <= Utc::now() {
        "expired"
    } else if row.approved_at.is_some() {
        "approved"
    } else {
        "pending"
    };
    Ok(Json(PairingView {
        pairing_id: row.id,
        device_name: row.device_name,
        expires_at: row.expires_at,
        status: status.into(),
    }))
}

pub async fn approve_pairing(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(pairing_id): Path<String>,
    Json(body): Json<ApprovePairingRequest>,
) -> Result<Json<ApprovePairingResponse>, ApiError> {
    require_jwt(&principal)?;
    let key = require_credential_key(state.config.local_mac_credential_key.as_ref())?;
    if !is_plausible_user_code(&body.user_code) {
        return Err(ApiError::Validation(
            "Could not approve this Mac. Check the code and try again.".into(),
        ));
    }
    let row = db::approve_pairing(
        &state.pool,
        key,
        &pairing_id,
        principal.owner_id(),
        &body.user_code,
    )
    .await?;
    Ok(Json(ApprovePairingResponse {
        pairing_id: row.id,
        device_name: row.device_name,
        status: "approved".into(),
    }))
}

pub async fn exchange_pairing(
    State(state): State<AppState>,
    Path(pairing_id): Path<String>,
    Json(body): Json<ExchangePairingRequest>,
) -> Result<Response, ApiError> {
    let key = require_credential_key(state.config.local_mac_credential_key.as_ref())?;
    let secret = body.pairing_secret.trim();
    if secret.is_empty() {
        return Err(ApiError::Unauthorized);
    }
    match db::exchange_pairing(&state.pool, key, &pairing_id, secret).await {
        Ok(issued) => Ok((
            StatusCode::OK,
            Json(ExchangePairingResponse {
                node_id: issued.node_id,
                computer_id: issued.computer_id,
                credential: issued.credential,
                display_name: issued.display_name,
            }),
        )
            .into_response()),
        Err(ExchangeFailure::Pending) => Ok((
            StatusCode::ACCEPTED,
            Json(StatusBody {
                status: "pending".into(),
            }),
        )
            .into_response()),
        Err(ExchangeFailure::Expired) => Err(ApiError::Conflict("pairing expired".into())),
        Err(ExchangeFailure::Consumed) => Err(ApiError::Conflict("pairing already used".into())),
        Err(ExchangeFailure::UnknownPairing | ExchangeFailure::InvalidSecret) => {
            Err(ApiError::Unauthorized)
        }
    }
}

pub async fn revoke_node(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(node_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_jwt(&principal)?;
    let revoked = db::revoke_node(&state.pool, principal.owner_id(), &node_id).await?;
    if revoked {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound)
    }
}
