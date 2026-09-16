use axum::extract::{Path, State};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::auth::Principal;
use crate::error::ApiError;
use crate::user_questions::UserQuestionRow;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserQuestionResponse {
    pub id: String,
    pub run_id: String,
    pub question: String,
    pub options: Vec<String>,
    pub status: String,
    pub selected_index: Option<i32>,
    pub selected_option: Option<String>,
    pub requested_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserQuestionStatusResponse {
    pub pending: Option<UserQuestionResponse>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnswerUserQuestionRequest {
    pub selected_index: usize,
}

fn to_response(row: UserQuestionRow) -> Result<UserQuestionResponse, ApiError> {
    let options = row
        .options
        .as_array()
        .ok_or_else(|| ApiError::Internal("question options missing".into()))?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_string)
                .ok_or_else(|| ApiError::Internal("question option invalid".into()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(UserQuestionResponse {
        id: row.id,
        run_id: row.run_id,
        question: row.question,
        options,
        status: row.status,
        selected_index: row.selected_index,
        selected_option: row.selected_option,
        requested_at: row.requested_at,
    })
}

pub async fn get_run_user_question(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(run_id): Path<String>,
) -> Result<Json<UserQuestionStatusResponse>, ApiError> {
    crate::db::queries::find_run_for_owner(&state.pool, principal.owner_id(), &run_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::NotFound)?;
    let row = state
        .user_questions
        .get_pending_for_owner_run(principal.owner_id(), &run_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(UserQuestionStatusResponse {
        pending: row.map(to_response).transpose()?,
    }))
}

pub async fn answer_user_question(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path((run_id, question_id)): Path<(String, String)>,
    Json(body): Json<AnswerUserQuestionRequest>,
) -> Result<Json<UserQuestionResponse>, ApiError> {
    let row = state
        .user_questions
        .answer(
            principal.owner_id(),
            &run_id,
            &question_id,
            body.selected_index,
        )
        .await?;
    Ok(Json(to_response(row)?))
}
