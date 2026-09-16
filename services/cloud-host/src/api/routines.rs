use crate::{
    auth::Principal,
    error::ApiError,
    routine_runs::RoutineRun,
    routine_webhooks::{self, WebhookTriggerView, WEBHOOK_MAX_BYTES},
    routines::{RoutineInput, RoutineView},
    AppState,
};
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Extension, Json,
};

fn public_origin(state: &AppState) -> Option<&str> {
    state.config.cors_web_origin.as_deref()
}

pub async fn list(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
) -> Result<Json<Vec<RoutineView>>, ApiError> {
    Ok(Json(
        crate::routines::list(&state.pool, owner.owner_id()).await?,
    ))
}

pub async fn get(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<String>,
) -> Result<Json<RoutineView>, ApiError> {
    Ok(Json(
        crate::routines::get(&state.pool, owner.owner_id(), &id).await?,
    ))
}

pub async fn create(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Json(input): Json<RoutineInput>,
) -> Result<Json<RoutineView>, ApiError> {
    Ok(Json(
        crate::routines::save_with_origin(
            &state.pool,
            owner.owner_id(),
            None,
            &input,
            public_origin(&state),
        )
        .await?,
    ))
}

pub async fn update(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<String>,
    Json(input): Json<RoutineInput>,
) -> Result<Json<RoutineView>, ApiError> {
    Ok(Json(
        crate::routines::save_with_origin(
            &state.pool,
            owner.owner_id(),
            Some(&id),
            &input,
            public_origin(&state),
        )
        .await?,
    ))
}

pub async fn delete(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<String>,
) -> Result<(), ApiError> {
    crate::routines::delete(&state.pool, owner.owner_id(), &id).await?;
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnabledInput {
    pub enabled: bool,
}

pub async fn set_enabled(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<String>,
    Json(input): Json<EnabledInput>,
) -> Result<Json<RoutineView>, ApiError> {
    Ok(Json(
        crate::routines::set_enabled(&state.pool, owner.owner_id(), &id, input.enabled).await?,
    ))
}

pub async fn test_run(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let key = headers
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let run_id = crate::routines::test_run(&state.pool, owner.owner_id(), &id, &key).await?;
    Ok(Json(serde_json::json!({ "runId": run_id })))
}

pub async fn run_now(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    test_run(State(state), Extension(owner), Path(id), headers).await
}

pub async fn list_runs(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<String>,
) -> Result<Json<Vec<RoutineRun>>, ApiError> {
    Ok(Json(
        crate::routines::list_runs(&state.pool, owner.owner_id(), &id).await?,
    ))
}

pub async fn get_webhook(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<String>,
) -> Result<Json<WebhookTriggerView>, ApiError> {
    Ok(Json(
        routine_webhooks::get_for_owner(&state.pool, owner.owner_id(), &id).await?,
    ))
}

pub async fn create_webhook(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<String>,
) -> Result<Json<WebhookTriggerView>, ApiError> {
    Ok(Json(
        routine_webhooks::create_for_owner(
            &state.pool,
            owner.owner_id(),
            &id,
            public_origin(&state),
        )
        .await?,
    ))
}

pub async fn rotate_webhook(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<String>,
) -> Result<Json<WebhookTriggerView>, ApiError> {
    Ok(Json(
        routine_webhooks::rotate_for_owner(
            &state.pool,
            owner.owner_id(),
            &id,
            public_origin(&state),
        )
        .await?,
    ))
}

pub async fn delete_webhook(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<String>,
) -> Result<(), ApiError> {
    routine_webhooks::delete_for_owner(&state.pool, owner.owner_id(), &id).await?;
    Ok(())
}

fn header_text<'a>(headers: &'a HeaderMap, name: &'static str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

fn content_type_is_json(headers: &HeaderMap) -> bool {
    headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(';')
                .next()
                .map(str::trim)
                .is_some_and(|media| media.eq_ignore_ascii_case("application/json"))
        })
}

pub async fn admit_public_webhook(
    State(state): State<AppState>,
    Path(token): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    if body.len() > WEBHOOK_MAX_BYTES {
        return Err(ApiError::PayloadTooLarge);
    }
    if !content_type_is_json(&headers) {
        return Err(ApiError::UnsupportedMediaType);
    }
    let payload: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|_| ApiError::Validation("JSON is required".into()))?;
    let event_id = routine_webhooks::event_id_from_headers(
        header_text(&headers, "Idempotency-Key"),
        header_text(&headers, "X-GitHub-Delivery"),
        header_text(&headers, "X-Request-Id"),
    )
    .map(str::to_string);
    let event_source = routine_webhooks::sanitize_event_source(
        header_text(&headers, "X-Elsewhere-Event-Source")
            .or_else(|| header_text(&headers, "X-GitHub-Event")),
    );
    crate::routines::admit_webhook_event(
        &state.pool,
        &token,
        payload,
        event_id.as_deref(),
        event_source.as_deref(),
    )
    .await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "accepted": true })),
    ))
}
