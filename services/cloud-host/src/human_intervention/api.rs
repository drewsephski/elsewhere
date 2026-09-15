use axum::extract::{Path, State};
use axum::Extension;
use axum::Json;
use serde::Serialize;

use crate::app_state::AppState;
use crate::auth::Principal;
use crate::error::ApiError;
use crate::human_intervention::HumanInterventionRow;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingHumanInterventionResponse {
    pub id: String,
    pub run_id: String,
    pub computer_id: String,
    pub reason: String,
    pub message: String,
    pub requested_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HumanInterventionStatusResponse {
    pub pending: Option<PendingHumanInterventionResponse>,
}

fn to_pending(row: HumanInterventionRow) -> PendingHumanInterventionResponse {
    PendingHumanInterventionResponse {
        id: row.id,
        run_id: row.run_id,
        computer_id: row.computer_id,
        reason: row.reason,
        message: row.message,
        requested_at: row.requested_at,
    }
}

pub async fn get_run_human_intervention(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(run_id): Path<String>,
) -> Result<Json<HumanInterventionStatusResponse>, ApiError> {
    let row = state
        .human_interventions
        .get_pending_for_owner_run(principal.owner_id(), &run_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(HumanInterventionStatusResponse {
        pending: row.map(to_pending),
    }))
}
