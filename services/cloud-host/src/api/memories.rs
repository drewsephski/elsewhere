use axum::extract::{Path, Query, State};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::auth::Principal;
use crate::error::ApiError;
use crate::memory::db::{
    archive_memory, insert_memory, list_memories, update_memory, MemoryFilters, MemoryRecord,
    NewMemory,
};
use crate::memory::service::require_owned_bot;
use crate::memory::types::{MemoryKind, MemorySourceKind};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryResponse {
    pub id: String,
    pub kind: String,
    pub content: String,
    pub source_kind: String,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_confirmed_at: chrono::DateTime<chrono::Utc>,
}

fn to_response(record: MemoryRecord) -> MemoryResponse {
    MemoryResponse {
        id: record.id,
        kind: record.kind,
        content: record.content,
        source_kind: record.source_kind,
        status: record.status,
        created_at: record.created_at,
        last_confirmed_at: record.last_confirmed_at,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListMemoriesQuery {
    pub query: Option<String>,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn list(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(bot_id): Path<String>,
    Query(query): Query<ListMemoriesQuery>,
) -> Result<Json<Vec<MemoryResponse>>, ApiError> {
    require_owned_bot(&state.pool, principal.owner_id(), &bot_id).await?;
    let records = list_memories(
        &state.pool,
        principal.owner_id(),
        &bot_id,
        &MemoryFilters {
            query: query.query,
            kind: query.kind,
            status: query.status,
            limit: query.limit.unwrap_or(50),
            offset: query.offset.unwrap_or(0),
        },
    )
    .await?;
    Ok(Json(records.into_iter().map(to_response).collect()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMemoryRequest {
    pub content: String,
    pub kind: Option<String>,
}

pub async fn create(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(bot_id): Path<String>,
    Json(body): Json<CreateMemoryRequest>,
) -> Result<Json<MemoryResponse>, ApiError> {
    require_owned_bot(&state.pool, principal.owner_id(), &bot_id).await?;
    let kind = body
        .kind
        .as_deref()
        .map(MemoryKind::parse)
        .transpose()
        .map_err(ApiError::Validation)?
        .unwrap_or(MemoryKind::Fact);
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let record = insert_memory(
        &mut tx,
        NewMemory {
            owner_id: principal.owner_id().to_string(),
            bot_id,
            kind,
            content: body.content,
            search_terms: Vec::new(),
            importance: 4,
            confidence: 1.0,
            source_kind: MemorySourceKind::Manual,
            source_run_id: None,
            source_message_id: None,
        },
    )
    .await?;
    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(to_response(record)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchMemoryRequest {
    pub content: Option<String>,
    pub kind: Option<String>,
}

pub async fn patch(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path((bot_id, memory_id)): Path<(String, String)>,
    Json(body): Json<PatchMemoryRequest>,
) -> Result<Json<MemoryResponse>, ApiError> {
    require_owned_bot(&state.pool, principal.owner_id(), &bot_id).await?;
    let kind = body
        .kind
        .as_deref()
        .map(MemoryKind::parse)
        .transpose()
        .map_err(ApiError::Validation)?;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let record = update_memory(
        &mut tx,
        principal.owner_id(),
        &bot_id,
        &memory_id,
        body.content.as_deref(),
        kind,
        None,
        None,
    )
    .await?;
    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(to_response(record)))
}

pub async fn delete(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path((bot_id, memory_id)): Path<(String, String)>,
) -> Result<Json<MemoryResponse>, ApiError> {
    require_owned_bot(&state.pool, principal.owner_id(), &bot_id).await?;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let record = archive_memory(&mut tx, principal.owner_id(), &bot_id, &memory_id).await?;
    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(to_response(record)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchMemoriesQuery {
    pub query: String,
    pub limit: Option<i64>,
}

pub async fn search(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(bot_id): Path<String>,
    Query(query): Query<SearchMemoriesQuery>,
) -> Result<Json<Vec<MemoryResponse>>, ApiError> {
    require_owned_bot(&state.pool, principal.owner_id(), &bot_id).await?;
    let records = crate::memory::search_memories(
        &state.pool,
        principal.owner_id(),
        &bot_id,
        &query.query,
        query.limit.unwrap_or(15),
    )
    .await?;
    Ok(Json(records.into_iter().map(to_response).collect()))
}
