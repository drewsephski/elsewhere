use axum::extract::{Extension, Path, State};
use axum::Json;

use crate::app_state::AppState;
use crate::auth::Principal;
use crate::delegation::{get_delegation, list_for_run, DelegationDetail};
use crate::error::ApiError;

pub async fn list_run_delegations(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(run_id): Path<String>,
) -> Result<Json<Vec<DelegationDetail>>, ApiError> {
    let rows = list_for_run(&state.pool, principal.owner_id(), &run_id).await?;
    Ok(Json(rows))
}

pub async fn get_delegation_by_id(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<String>,
) -> Result<Json<DelegationDetail>, ApiError> {
    let row = get_delegation(&state.pool, principal.owner_id(), &id).await?;
    Ok(Json(row))
}
