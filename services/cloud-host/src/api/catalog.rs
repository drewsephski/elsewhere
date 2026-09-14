use axum::extract::{Query, State};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::auth::Principal;
use crate::db::resources::{list_conversations_for_owner, list_runs_for_owner};
use crate::error::ApiError;

#[derive(Debug, Deserialize)]
pub struct ListRunsQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummaryResponse {
    pub run_id: String,
    pub request_id: String,
    pub bot_id: String,
    pub conversation_id: String,
    pub status: String,
    pub model: String,
    pub computer_id: Option<String>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn list_runs(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Query(query): Query<ListRunsQuery>,
) -> Result<Json<Vec<RunSummaryResponse>>, ApiError> {
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let rows = list_runs_for_owner(&state.pool, principal.owner_id(), limit)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(
        rows.into_iter()
            .map(|run| RunSummaryResponse {
                run_id: run.id,
                request_id: run.request_id,
                bot_id: run.bot_id,
                conversation_id: run.conversation_id,
                status: run.status,
                model: run.model,
                computer_id: run.computer_id,
                started_at: run.started_at,
                finished_at: run.finished_at,
            })
            .collect(),
    ))
}

#[derive(Debug, Deserialize)]
pub struct ListConversationsQuery {
    pub bot_id: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummaryResponse {
    pub id: String,
    pub bot_id: String,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_conversations(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Query(query): Query<ListConversationsQuery>,
) -> Result<Json<Vec<ConversationSummaryResponse>>, ApiError> {
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let rows = list_conversations_for_owner(
        &state.pool,
        principal.owner_id(),
        query.bot_id.as_deref(),
        limit,
    )
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(
        rows.into_iter()
            .map(|row| ConversationSummaryResponse {
                id: row.id,
                bot_id: row.bot_id,
                updated_at: row.updated_at,
            })
            .collect(),
    ))
}
