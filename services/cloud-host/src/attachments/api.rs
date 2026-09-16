use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Multipart, Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::Serialize;

use super::store;
use super::validate::validate_upload_bytes;
use super::UPLOAD_BODY_LIMIT_BYTES;
use crate::app_state::AppState;
use crate::auth::Principal;
use crate::error::ApiError;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentResponse {
    pub id: String,
    pub original_name: String,
    pub safe_name: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub status: String,
}

pub fn upload_routes() -> Router<AppState> {
    Router::new()
        .route("/v1/bots/{bot_id}/attachments", post(upload_bot_attachment))
        .route(
            "/v1/conversations/{conversation_id}/attachments",
            post(upload_conversation_attachment),
        )
        .layer(DefaultBodyLimit::max(UPLOAD_BODY_LIMIT_BYTES))
}

pub fn download_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/attachments/{id}",
            get(get_attachment_meta).delete(delete_attachment),
        )
        .route("/v1/attachments/{id}/content", get(download_attachment))
}

async fn upload_bot_attachment(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(bot_id): Path<String>,
    multipart: Multipart,
) -> Result<(StatusCode, Json<AttachmentResponse>), ApiError> {
    let owned: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM bots WHERE id = $1 AND owner_id = $2)")
            .bind(&bot_id)
            .bind(principal.owner_id())
            .fetch_one(&state.pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    if !owned {
        return Err(ApiError::NotFound);
    }
    persist_upload(
        &state,
        principal.owner_id(),
        Some(bot_id.as_str()),
        None,
        multipart,
    )
    .await
}

async fn upload_conversation_attachment(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(conversation_id): Path<String>,
    multipart: Multipart,
) -> Result<(StatusCode, Json<AttachmentResponse>), ApiError> {
    crate::groups::get_conversation_for_owner(&state.pool, principal.owner_id(), &conversation_id)
        .await?;
    persist_upload(
        &state,
        principal.owner_id(),
        None,
        Some(conversation_id.as_str()),
        multipart,
    )
    .await
}

async fn persist_upload(
    state: &AppState,
    owner_id: &str,
    bot_id: Option<&str>,
    conversation_id: Option<&str>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<AttachmentResponse>), ApiError> {
    let mut file_name = String::new();
    let mut claimed_mime: Option<String> = None;
    let mut bytes: Option<Bytes> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| ApiError::Validation("invalid multipart upload".into()))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name != "file" && name != "attachment" {
            continue;
        }
        file_name = field.file_name().unwrap_or("file").to_string();
        claimed_mime = field.content_type().map(str::to_string);
        bytes = Some(field.bytes().await.map_err(|_| ApiError::PayloadTooLarge)?);
        break;
    }
    let Some(bytes) = bytes else {
        return Err(ApiError::Validation("file is required".into()));
    };
    if bytes.len() > UPLOAD_BODY_LIMIT_BYTES {
        return Err(ApiError::PayloadTooLarge);
    }
    let validated = validate_upload_bytes(&file_name, claimed_mime.as_deref(), &bytes)
        .map_err(|e| ApiError::Validation(e.message().into()))?;
    let row = store::insert_staged(
        &state.pool,
        owner_id,
        bot_id,
        conversation_id,
        &validated,
        &bytes,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(to_response(&row))))
}

async fn get_attachment_meta(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<String>,
) -> Result<Json<AttachmentResponse>, ApiError> {
    let row = store::load_meta(&state.pool, principal.owner_id(), &id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(to_response(&row)))
}

async fn download_attachment(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let (row, content) = store::load_content_for_owner(&state.pool, principal.owner_id(), &id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        row.mime_type
            .parse()
            .unwrap_or(header::HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("inline; filename=\"{}\"", row.safe_name.replace('"', ""))
            .parse()
            .unwrap_or(header::HeaderValue::from_static("inline")),
    );
    headers.insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("private, no-store"),
    );
    Ok((headers, content))
}

async fn delete_attachment(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    if store::delete_staged(&state.pool, principal.owner_id(), &id).await? {
        return Ok(StatusCode::NO_CONTENT);
    }
    if store::hide_attached(&state.pool, principal.owner_id(), &id).await? {
        return Ok(StatusCode::NO_CONTENT);
    }
    Err(ApiError::NotFound)
}

fn to_response(row: &store::AttachmentRow) -> AttachmentResponse {
    AttachmentResponse {
        id: row.id.clone(),
        original_name: row.original_name.clone(),
        safe_name: row.safe_name.clone(),
        mime_type: row.mime_type.clone(),
        size_bytes: row.size_bytes as u64,
        sha256: row.sha256.clone(),
        status: row.status.clone(),
    }
}
