use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use thiserror::Error;

use crate::redact::redact_secrets;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("validation: {0}")]
    Validation(String),
    #[error("not found")]
    NotFound,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("too many requests")]
    TooManyRequests,
    #[error("payload too large")]
    PayloadTooLarge,
    #[error("unsupported media type")]
    UnsupportedMediaType,
    #[error("internal: {0}")]
    Internal(String),
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            ApiError::Validation(m) => (StatusCode::BAD_REQUEST, format!("validation: {m}")),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not found".into()),
            ApiError::Conflict(m) => (StatusCode::CONFLICT, format!("conflict: {m}")),
            ApiError::TooManyRequests => (
                StatusCode::TOO_MANY_REQUESTS,
                "too many concurrent runs".into(),
            ),
            ApiError::PayloadTooLarge => {
                (StatusCode::PAYLOAD_TOO_LARGE, "payload too large".into())
            }
            ApiError::UnsupportedMediaType => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "JSON is required".into(),
            ),
            ApiError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m.clone()),
        };
        let safe = redact_secrets(&msg);
        (status, Json(ErrorBody { error: safe })).into_response()
    }
}
