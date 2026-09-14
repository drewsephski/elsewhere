use axum::extract::{Query, State};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::auth::Principal;
use crate::db::resources::list_conversations_for_owner;
use crate::error::ApiError;

#[derive(Debug, Deserialize)]
pub struct ListRunsQuery {
    pub bot_id: Option<String>,
    pub limit: Option<i64>,
    /// When true, return only archived runs. When false or omitted, return active runs only.
    pub archived: Option<bool>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct RunSummaryResponse {
    pub task: String,
    pub bot_name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub run_id: String,
    pub request_id: String,
    pub bot_id: String,
    pub conversation_id: String,
    pub status: String,
    pub model: String,
    pub computer_id: Option<String>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    pub archived_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn list_runs(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Query(query): Query<ListRunsQuery>,
) -> Result<Json<Vec<RunSummaryResponse>>, ApiError> {
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let archived_only = query.archived.unwrap_or(false);
    let rows = sqlx::query_as::<_,RunSummaryResponse>(
        "SELECT r.id AS run_id, r.request_id, r.bot_id, r.conversation_id, r.status, r.model, r.computer_id, r.started_at, r.finished_at, r.archived_at, r.created_at, b.name AS bot_name, LEFT(COALESCE(q.user_message, 'Delegated work'), 180) AS task \
         FROM agent_runs r \
         JOIN bots b ON b.id = r.bot_id \
         LEFT JOIN work_queue q ON q.run_id = r.id \
         WHERE r.owner_id = $1 \
         AND ($3::text IS NULL OR r.bot_id = $3) \
         AND (($4::bool AND r.archived_at IS NOT NULL) OR (NOT $4::bool AND r.archived_at IS NULL)) \
         ORDER BY r.created_at DESC LIMIT $2",
    )
        .bind(principal.owner_id()).bind(limit).bind(query.bot_id).bind(archived_only).fetch_all(&state.pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(rows))
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
