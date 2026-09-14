mod jwt;
mod principal;

#[cfg(any(test, feature = "test-utils"))]
pub mod jwt_test;

pub use jwt::{JwtVerifier, JwtVerifierConfig};
pub use principal::{AuthKind, Principal, LEGACY_LOCAL_OWNER};

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;

use crate::app_state::AppState;
use crate::config::AuthMode;
use crate::error::ApiError;

fn bearer_token(auth_header: &str) -> Option<&str> {
    auth_header
        .strip_prefix("Bearer ")
        .or_else(|| auth_header.strip_prefix("bearer "))
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

fn looks_like_jwt(token: &str) -> bool {
    token.matches('.').count() == 2
}

pub async fn require_authenticated(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let auth = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let token = bearer_token(auth).ok_or(ApiError::Unauthorized)?;

    let principal = match state.config.auth_mode {
        AuthMode::InternalToken => authenticate_internal(&state, token)?,
        AuthMode::Jwt => authenticate_jwt(&state, token).await?,
        AuthMode::Hybrid => {
            if constant_time_eq(token, &state.config.api_token) {
                Principal::legacy_local()
            } else if looks_like_jwt(token) {
                authenticate_jwt(&state, token).await?
            } else {
                return Err(ApiError::Unauthorized);
            }
        }
    };

    request.extensions_mut().insert(principal);
    Ok(next.run(request).await)
}

fn authenticate_internal(state: &AppState, token: &str) -> Result<Principal, ApiError> {
    if constant_time_eq(token, &state.config.api_token) {
        Ok(Principal::legacy_local())
    } else {
        Err(ApiError::Unauthorized)
    }
}

async fn authenticate_jwt(state: &AppState, token: &str) -> Result<Principal, ApiError> {
    let verifier = state
        .jwt_verifier
        .as_ref()
        .ok_or(ApiError::Internal("jwt auth not configured".into()))?;
    let sub = verifier.verify_bearer_token(token).await?;
    if sub == LEGACY_LOCAL_OWNER || sub.trim().is_empty() {
        return Err(ApiError::Unauthorized);
    }
    Ok(Principal {
        subject: sub,
        auth_kind: AuthKind::Jwt,
    })
}

fn constant_time_eq(got: &str, expected: &str) -> bool {
    if got.len() != expected.len() {
        return false;
    }
    got.as_bytes()
        .iter()
        .zip(expected.as_bytes())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

pub fn extension_principal(request: &Request) -> Option<&Principal> {
    request.extensions().get::<Principal>()
}

/// Reject JWT principals on legacy-only internal bootstrap endpoints.
pub fn require_internal_token(principal: &Principal) -> Result<(), ApiError> {
    if principal.auth_kind == AuthKind::InternalToken {
        Ok(())
    } else {
        Err(ApiError::Unauthorized)
    }
}
