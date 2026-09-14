use axum::extract::{Path, State};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::auth::Principal;
use crate::db::resources::{
    archive_computer, get_computer_for_owner, insert_computer_placeholder, list_computers,
};
use crate::error::ApiError;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerResponse {
    pub id: String,
    pub display_name: String,
    pub provider: String,
    pub state: String,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    pub provider_metadata: ProviderMetadata,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderMetadata {
    pub provisioned: bool,
}

fn to_response(row: crate::db::resources::SandboxRow) -> ComputerResponse {
    ComputerResponse {
        id: row.id.clone(),
        display_name: if row.display_name.is_empty() {
            row.id.clone()
        } else {
            row.display_name
        },
        provider: row.provider,
        state: row.state.clone(),
        last_used_at: row.last_used_at,
        provider_metadata: ProviderMetadata {
            provisioned: row.state == "active",
        },
    }
}

pub async fn list(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
) -> Result<Json<Vec<ComputerResponse>>, ApiError> {
    let rows = list_computers(&state.pool, principal.owner_id())
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(rows.into_iter().map(to_response).collect()))
}

#[derive(Debug, Deserialize)]
pub struct CreateComputerRequest {
    #[serde(rename = "displayName")]
    pub display_name: String,
}

pub async fn create(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(body): Json<CreateComputerRequest>,
) -> Result<Json<ComputerResponse>, ApiError> {
    if body.display_name.trim().is_empty() || body.display_name.len() > 100 {
        return Err(ApiError::Validation("Computer names must contain 1 to 100 bytes".into()));
    }
    let row =
        insert_computer_placeholder(&state.pool, principal.owner_id(), body.display_name.trim())
            .await?;
    Ok(Json(to_response(row)))
}

pub async fn get(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(computer_id): Path<String>,
) -> Result<Json<ComputerResponse>, ApiError> {
    let row = get_computer_for_owner(&state.pool, principal.owner_id(), &computer_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(to_response(row)))
}

pub async fn delete(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(computer_id): Path<String>,
) -> Result<axum::http::StatusCode, ApiError> {
    let archived = archive_computer(&state.pool, principal.owner_id(), &computer_id).await?;
    if archived {
        Ok(axum::http::StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound)
    }
}
