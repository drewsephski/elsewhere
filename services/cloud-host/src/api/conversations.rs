use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::auth::Principal;
use crate::error::ApiError;
use crate::groups::{
    add_participant, append_human_message, create_group, enqueue_group_bot_run,
    get_conversation_for_owner, list_groups, list_messages, remove_participant, send_group_message,
    CreateGroupRequest, GroupConversationDetail, GroupListItem, SendGroupMessageRequest,
    SendGroupMessageResponse, TranscriptMessage,
};

pub async fn get_conversation(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(conversation_id): Path<String>,
) -> Result<Json<GroupConversationDetail>, ApiError> {
    let detail =
        get_conversation_for_owner(&state.pool, principal.owner_id(), &conversation_id).await?;
    Ok(Json(detail))
}

pub async fn list_conversation_messages(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(conversation_id): Path<String>,
) -> Result<Json<Vec<TranscriptMessage>>, ApiError> {
    let messages =
        list_messages(&state.pool, principal.owner_id(), &conversation_id).await?;
    Ok(Json(messages))
}

pub async fn create_group_conversation(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(body): Json<CreateGroupRequest>,
) -> Result<(StatusCode, Json<GroupConversationDetail>), ApiError> {
    let detail = create_group(&state.pool, principal.owner_id(), body).await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppendHumanMessageRequest {
    pub body: String,
    pub recipient_bot_ids: Option<Vec<String>>,
    pub mention_mode: Option<String>,
}

pub async fn append_human_message_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(conversation_id): Path<String>,
    headers: axum::http::HeaderMap,
    Json(body): Json<AppendHumanMessageRequest>,
) -> Result<(StatusCode, Json<SendGroupMessageResponse>), ApiError> {
    let has_routing = body.mention_mode.is_some()
        || body
            .recipient_bot_ids
            .as_ref()
            .is_some_and(|ids| ids.iter().any(|id| !id.trim().is_empty()));
    if has_routing {
        let idempotency_key = headers
            .get("Idempotency-Key")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let response = send_group_message(
            &state.pool,
            principal.owner_id(),
            &conversation_id,
            &idempotency_key,
            SendGroupMessageRequest {
                body: body.body,
                recipient_bot_ids: body.recipient_bot_ids,
                mention_mode: body.mention_mode,
            },
        )
        .await?;
        return Ok((StatusCode::CREATED, Json(response)));
    }
    let message = append_human_message(
        &state.pool,
        principal.owner_id(),
        &conversation_id,
        &body.body,
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(SendGroupMessageResponse {
            message,
            recipients: Vec::new(),
        }),
    ))
}

pub async fn list_group_conversations(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
) -> Result<Json<Vec<GroupListItem>>, ApiError> {
    let groups = list_groups(&state.pool, principal.owner_id(), 50).await?;
    Ok(Json(groups))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddParticipantRequest {
    pub bot_id: String,
}

pub async fn add_participant_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(conversation_id): Path<String>,
    Json(body): Json<AddParticipantRequest>,
) -> Result<Json<GroupConversationDetail>, ApiError> {
    let detail = add_participant(
        &state.pool,
        principal.owner_id(),
        &conversation_id,
        body.bot_id.trim(),
    )
    .await?;
    Ok(Json(detail))
}

pub async fn remove_participant_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path((conversation_id, bot_id)): Path<(String, String)>,
) -> Result<Json<GroupConversationDetail>, ApiError> {
    let detail = remove_participant(
        &state.pool,
        principal.owner_id(),
        &conversation_id,
        bot_id.trim(),
    )
    .await?;
    Ok(Json(detail))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupRunRequest {
    pub bot_id: String,
    pub message: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupRunResponse {
    pub run_id: String,
    pub request_id: String,
    pub conversation_id: String,
    pub bot_id: String,
}

pub async fn enqueue_group_run(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(conversation_id): Path<String>,
    headers: axum::http::HeaderMap,
    Json(body): Json<GroupRunRequest>,
) -> Result<(StatusCode, Json<GroupRunResponse>), ApiError> {
    if body.message.trim().is_empty() {
        return Err(ApiError::Validation("message cannot be empty".into()));
    }
    if body.bot_id.trim().is_empty() {
        return Err(ApiError::Validation("botId is required".into()));
    }
    let request_id = headers
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let records = enqueue_group_bot_run(
        &state.pool,
        principal.owner_id(),
        &conversation_id,
        body.bot_id.trim(),
        &request_id,
        body.message.trim(),
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(GroupRunResponse {
            run_id: records.run_id,
            request_id: records.request_id,
            conversation_id: records.conversation_id,
            bot_id: body.bot_id.trim().to_string(),
        }),
    ))
}
