use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

use crate::app_state::AppState;
use crate::error::ApiError;

pub async fn require_bearer(
    axum::extract::State(state): axum::extract::State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let auth = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let expected = format!("Bearer {}", state.config.api_token);
    if auth.len() != expected.len()
        || auth
            .as_bytes()
            .iter()
            .zip(expected.as_bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            != 0
    {
        return Err(ApiError::Unauthorized);
    }
    Ok(next.run(request).await)
}
