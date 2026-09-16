use axum::{
    extract::{Path, State},
    Extension, Json,
};
use serde::Deserialize;

use crate::app_state::AppState;
use crate::auth::Principal;
use crate::error::ApiError;
use crate::permission_policies::{PolicyCatalogResponse, PolicyDecision};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyPatchItem {
    pub action: String,
    pub decision: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyPatchRequest {
    pub policies: Vec<PolicyPatchItem>,
}

fn parse_optional_decision(raw: Option<&str>) -> Result<Option<PolicyDecision>, ApiError> {
    match raw {
        None => Ok(None),
        Some(value) if value.is_empty() => Ok(None),
        Some(value) => Ok(Some(PolicyDecision::parse(value)?)),
    }
}

pub async fn get_owner_policies(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
) -> Result<Json<PolicyCatalogResponse>, ApiError> {
    let catalog = state
        .permission_policies
        .owner_catalog(principal.owner_id())
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(catalog))
}

pub async fn put_owner_policies(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(body): Json<PolicyPatchRequest>,
) -> Result<Json<PolicyCatalogResponse>, ApiError> {
    for item in &body.policies {
        let decision = parse_optional_decision(item.decision.as_deref())?;
        state
            .permission_policies
            .upsert_owner_decision(principal.owner_id(), item.action.trim(), decision)
            .await?;
    }
    let catalog = state
        .permission_policies
        .owner_catalog(principal.owner_id())
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(catalog))
}

pub async fn get_bot_policies(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(bot_id): Path<String>,
) -> Result<Json<PolicyCatalogResponse>, ApiError> {
    state
        .permission_policies
        .bot_catalog(principal.owner_id(), &bot_id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound)
}

pub async fn put_bot_policies(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(bot_id): Path<String>,
    Json(body): Json<PolicyPatchRequest>,
) -> Result<Json<PolicyCatalogResponse>, ApiError> {
    for item in &body.policies {
        let decision = parse_optional_decision(item.decision.as_deref())?;
        let found = state
            .permission_policies
            .upsert_bot_decision(principal.owner_id(), &bot_id, item.action.trim(), decision)
            .await?;
        if !found {
            return Err(ApiError::NotFound);
        }
    }
    state
        .permission_policies
        .bot_catalog(principal.owner_id(), &bot_id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound)
}
