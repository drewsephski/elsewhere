use axum::extract::{Path, State};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::auth::Principal;
use crate::bot_avatar::normalize_avatar_id;
use crate::db::resources::{
    delete_bot, get_bot_for_owner, insert_bot, list_bots, normalize_engine_preference,
    normalize_model, patch_bot,
};
use crate::error::ApiError;

fn validate_settings(name: Option<&str>, instructions: Option<&str>) -> Result<(), ApiError> {
    if name.is_some_and(|value| value.trim().is_empty() || value.len() > 100) {
        return Err(ApiError::Validation(
            "Bot names must contain 1 to 100 bytes".into(),
        ));
    }
    if instructions.is_some_and(|value| value.len() > 16_000) {
        return Err(ApiError::Validation(
            "Bot instructions must be at most 16,000 bytes".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BotResponse {
    pub id: String,
    pub name: String,
    pub instructions: String,
    pub model: String,
    pub computer_id: Option<String>,
    pub engine_preference: String,
    pub avatar_id: String,
    pub learn_from_conversations: bool,
}

fn to_response(row: crate::db::resources::BotRow) -> BotResponse {
    BotResponse {
        id: row.id,
        name: row.name,
        instructions: row.system_prompt,
        model: row.model,
        computer_id: row.computer_id,
        engine_preference: row.engine_preference,
        avatar_id: row.avatar_id,
        learn_from_conversations: row.learn_from_conversations,
    }
}

pub async fn list(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
) -> Result<Json<Vec<BotResponse>>, ApiError> {
    let rows = list_bots(&state.pool, principal.owner_id())
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(rows.into_iter().map(to_response).collect()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBotRequest {
    pub name: String,
    pub instructions: String,
    pub model: Option<String>,
    pub computer_id: Option<String>,
    pub engine_preference: Option<String>,
    pub avatar_id: Option<String>,
}

pub async fn create(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(body): Json<CreateBotRequest>,
) -> Result<Json<BotResponse>, ApiError> {
    validate_settings(Some(&body.name), Some(&body.instructions))?;
    let engine = normalize_engine_preference(body.engine_preference.as_deref().unwrap_or("auto"))?;
    if let Some(computer_id) = body.computer_id.as_deref().filter(|c| !c.is_empty()) {
        let computer = crate::db::resources::get_computer_for_owner(
            &state.pool,
            principal.owner_id(),
            computer_id,
        )
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::Validation("computerId not found".into()))?;
        if computer.state == "archived" {
            return Err(ApiError::Validation("computer is archived".into()));
        }
    }
    let avatar_id = normalize_avatar_id(body.avatar_id.as_deref())?;
    let row = insert_bot(
        &state.pool,
        principal.owner_id(),
        body.name.trim(),
        body.instructions.trim(),
        &normalize_model(body.model.as_deref()),
        body.computer_id.as_deref(),
        engine,
        &avatar_id,
    )
    .await?;
    Ok(Json(to_response(row)))
}

pub async fn get(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(bot_id): Path<String>,
) -> Result<Json<BotResponse>, ApiError> {
    let row = get_bot_for_owner(&state.pool, principal.owner_id(), &bot_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(to_response(row)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchBotRequest {
    pub name: Option<String>,
    pub instructions: Option<String>,
    pub model: Option<String>,
    pub computer_id: Option<String>,
    pub engine_preference: Option<String>,
    pub avatar_id: Option<String>,
    pub learn_from_conversations: Option<bool>,
}

pub async fn patch(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(bot_id): Path<String>,
    Json(body): Json<PatchBotRequest>,
) -> Result<Json<BotResponse>, ApiError> {
    validate_settings(body.name.as_deref(), body.instructions.as_deref())?;
    let engine = match body.engine_preference.as_deref() {
        Some(raw) => Some(normalize_engine_preference(raw)?),
        None => None,
    };
    if let Some(computer_id) = body.computer_id.as_deref().filter(|c| !c.is_empty()) {
        let computer = crate::db::resources::get_computer_for_owner(
            &state.pool,
            principal.owner_id(),
            computer_id,
        )
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::Validation("computerId not found".into()))?;
        if computer.state == "archived" {
            return Err(ApiError::Validation("computer is archived".into()));
        }
    }
    let avatar_id = match body.avatar_id.as_deref() {
        Some(raw) => Some(normalize_avatar_id(Some(raw))?),
        None => None,
    };
    let model = body.model.as_deref().map(|raw| normalize_model(Some(raw)));
    let row = patch_bot(
        &state.pool,
        principal.owner_id(),
        &bot_id,
        body.name.as_deref().map(str::trim),
        body.instructions.as_deref().map(str::trim),
        model.as_deref(),
        body.computer_id
            .as_deref()
            .map(|id| if id.is_empty() { None } else { Some(id) }),
        engine,
        avatar_id.as_deref(),
        body.learn_from_conversations,
    )
    .await?
    .ok_or(ApiError::NotFound)?;
    Ok(Json(to_response(row)))
}

pub async fn delete(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(bot_id): Path<String>,
) -> Result<axum::http::StatusCode, ApiError> {
    let deleted = delete_bot(&state.pool, principal.owner_id(), &bot_id).await?;
    if deleted {
        Ok(axum::http::StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound)
    }
}
