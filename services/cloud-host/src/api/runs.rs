use axum::response::sse::{Event, KeepAlive, Sse};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use futures_util::stream::Stream;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::time::Duration;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::db::queries::{
    assistant_message_body, bootstrap_run, find_run_by_id, list_run_events_after,
};
use crate::error::ApiError;
use crate::runner::{spawn_agent_run, RunExecutionInput};

#[derive(Debug, Deserialize)]
pub struct CreateRunRequest {
    pub bot: BotPayload,
    #[serde(rename = "conversationId")]
    pub conversation_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct BotPayload {
    pub id: String,
    pub name: String,
    pub instructions: String,
    pub model: Option<String>,
    #[serde(rename = "computerId")]
    pub computer_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRunResponse {
    pub run_id: String,
    pub request_id: String,
    pub conversation_id: String,
    pub computer_id: String,
    pub model: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunDetailResponse {
    pub run_id: String,
    pub request_id: String,
    pub status: String,
    pub model: String,
    pub bot_id: String,
    pub computer_id: Option<String>,
    pub conversation_id: String,
    pub step_count: i64,
    pub error_code: Option<String>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    pub assistant_result: Option<String>,
}

pub async fn create_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateRunRequest>,
) -> Result<(StatusCode, Json<CreateRunResponse>), ApiError> {
    if body.message.trim().is_empty() {
        return Err(ApiError::Validation("message cannot be empty".into()));
    }
    if body.bot.id.trim().is_empty() || body.bot.computer_id.trim().is_empty() {
        return Err(ApiError::Validation("bot.id and bot.computerId are required".into()));
    }

    let request_id = headers
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    if state.run_semaphore.available_permits() == 0 {
        return Err(ApiError::TooManyRequests);
    }

    let model = body
        .bot
        .model
        .as_deref()
        .filter(|m| !m.trim().is_empty())
        .map(str::to_string);

    let records = bootstrap_run(
        &state.pool,
        &request_id,
        &body.bot.id,
        &body.bot.name,
        &body.bot.instructions,
        model.as_deref(),
        &body.bot.computer_id,
        body.conversation_id.as_deref(),
        body.message.trim(),
    )
    .await?;

    let run_row = find_run_by_id(&state.pool, &records.run_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::NotFound)?;

    let response = CreateRunResponse {
        run_id: records.run_id.clone(),
        request_id: records.request_id.clone(),
        conversation_id: records.conversation_id.clone(),
        computer_id: records.computer_id.clone(),
        model: records.model.clone(),
        status: run_row.status.clone(),
    };

    if run_row.status != "running" || state.registry.get(&records.run_id).is_some() {
        return Ok((StatusCode::ACCEPTED, Json(response)));
    }

    spawn_agent_run(
        state.clone(),
        RunExecutionInput {
            records,
            bot_id: body.bot.id,
            user_message: body.message.trim().to_string(),
        },
    );

    Ok((StatusCode::ACCEPTED, Json(response)))
}

pub async fn get_run(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> Result<Json<RunDetailResponse>, ApiError> {
    let run = find_run_by_id(&state.pool, &run_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::NotFound)?;

    let assistant_result = if let Some(id) = run.assistant_message_id.as_deref() {
        assistant_message_body(&state.pool, id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
    } else {
        None
    };

    Ok(Json(RunDetailResponse {
        run_id: run.id,
        request_id: run.request_id,
        status: run.status,
        model: run.model,
        bot_id: run.bot_id,
        computer_id: run.computer_id,
        conversation_id: run.conversation_id,
        step_count: run.step_count,
        error_code: run.error_code,
        started_at: run.started_at,
        finished_at: run.finished_at,
        assistant_result,
    }))
}

pub async fn cancel_run(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> Result<(StatusCode, Json<RunDetailResponse>), ApiError> {
    let run = find_run_by_id(&state.pool, &run_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::NotFound)?;

    if run.status == "running" {
        state.registry.cancel(&run_id);
    }

    let updated = get_run(State(state), Path(run_id)).await?;
    Ok((StatusCode::ACCEPTED, updated))
}

pub async fn run_events_sse(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let run = find_run_by_id(&state.pool, &run_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::NotFound)?;

    let after_id = headers
        .get("Last-Event-ID")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    let pool = state.pool.clone();
    let request_id = run.request_id.clone();
    let live = state.registry.subscribe_live(&run_id);

    let stream = async_stream::stream! {
        let durable = list_run_events_after(&pool, &request_id, after_id, 10_000)
            .await
            .unwrap_or_default();
        for row in durable {
            let data = row.payload_json.to_string();
            yield Ok(Event::default().id(row.id.to_string()).event(row.event_type).data(data));
        }

        if let Some(mut rx) = live {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        if let Ok(json) = serde_json::to_string(&event) {
                            yield Ok(Event::default().event("agent_event").data(json));
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        } else if !matches!(run.status.as_str(), "running") {
            let terminal = serde_json::json!({"status": run.status, "errorCode": run.error_code});
            yield Ok(Event::default().event("terminal").data(terminal.to_string()));
        }
    };

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_model_is_luna_when_omitted() {
        assert_eq!(agent_core::DEFAULT_MODEL, "gpt-5.6-luna");
    }
}
