use crate::{
    auth::Principal,
    error::ApiError,
    routines::{Routine, RoutineInput},
    AppState,
};
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Extension, Json,
};

pub async fn list(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
) -> Result<Json<Vec<Routine>>, ApiError> {
    Ok(Json(
        crate::routines::list(&state.pool, owner.owner_id()).await?,
    ))
}
pub async fn create(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Json(input): Json<RoutineInput>,
) -> Result<Json<Routine>, ApiError> {
    Ok(Json(
        crate::routines::save(&state.pool, owner.owner_id(), None, &input).await?,
    ))
}
pub async fn update(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<String>,
    Json(input): Json<RoutineInput>,
) -> Result<Json<Routine>, ApiError> {
    Ok(Json(
        crate::routines::save(&state.pool, owner.owner_id(), Some(&id), &input).await?,
    ))
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnabledInput {
    pub enabled: bool,
}
pub async fn set_enabled(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<String>,
    Json(input): Json<EnabledInput>,
) -> Result<Json<Routine>, ApiError> {
    Ok(Json(
        crate::routines::set_enabled(&state.pool, owner.owner_id(), &id, input.enabled).await?,
    ))
}
pub async fn run_now(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let key = headers
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let run_id = crate::routines::run_now(&state.pool, owner.owner_id(), &id, &key).await?;
    Ok(Json(serde_json::json!({"runId":run_id})))
}
