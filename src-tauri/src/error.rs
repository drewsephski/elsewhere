use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("missing API key: configure your OpenAI API key in Settings")]
    MissingApiKey,
    #[error("invalid API key")]
    InvalidApiKey,
    #[error("network error: {0}")]
    Network(String),
    #[error("provider error: {0}")]
    Provider(String),
    #[error("rate limited: {0}")]
    RateLimited(String),
    #[error("model unavailable: {0}")]
    ModelUnavailable(String),
    #[error("stream interrupted")]
    StreamInterrupted,
    #[error("request cancelled")]
    Cancelled,
    #[error("secret storage error: {0}")]
    Secret(String),
    #[error("{0}")]
    Other(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl AppError {
    pub fn from_openai_status(status: reqwest::StatusCode, body: &str) -> Self {
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return AppError::InvalidApiKey;
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return AppError::RateLimited(body.to_string());
        }
        if status == reqwest::StatusCode::NOT_FOUND {
            return AppError::ModelUnavailable(body.to_string());
        }
        AppError::Provider(format!("HTTP {}: {}", status, body))
    }
}
