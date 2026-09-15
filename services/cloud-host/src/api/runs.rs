use axum::response::sse::{Event, KeepAlive, Sse};
use axum::{
    extract::{Extension, Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use futures_util::stream::Stream;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::time::Duration;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::auth::{require_internal_token, AuthKind, Principal};
use crate::db::queries::{
    assistant_message_body, bootstrap_run_legacy, find_run_for_owner, list_run_events_after,
};
use crate::error::ApiError;
use crate::runner::{spawn_agent_run, RunExecutionInput};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillInvocationInput {
    pub skill_id: String,
    pub skill_version: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductCreateRunRequest {
    pub bot_id: String,
    pub conversation_id: Option<String>,
    pub message: String,
    pub skill_invocation: Option<SkillInvocationInput>,
}

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
    pub task: Option<String>,
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
    Extension(principal): Extension<Principal>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<(StatusCode, Json<CreateRunResponse>), ApiError> {
    let request_id = headers
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    if principal.auth_kind == AuthKind::Jwt {
        let product: ProductCreateRunRequest = serde_json::from_slice(&body)
            .map_err(|_| ApiError::Validation("invalid product run payload".into()))?;
        return create_product_run(state, principal, request_id, product).await;
    }

    require_internal_token(&principal)?;
    let legacy: CreateRunRequest = serde_json::from_slice(&body)
        .map_err(|_| ApiError::Validation("invalid legacy run payload".into()))?;
    create_legacy_run(state, request_id, legacy).await
}

async fn create_product_run(
    state: AppState,
    principal: Principal,
    request_id: String,
    body: ProductCreateRunRequest,
) -> Result<(StatusCode, Json<CreateRunResponse>), ApiError> {
    if body.message.trim().is_empty() {
        return Err(ApiError::Validation("message cannot be empty".into()));
    }
    if body.bot_id.trim().is_empty() {
        return Err(ApiError::Validation("botId is required".into()));
    }

    let mut skills = crate::skills::SkillAdmissionInput::default();
    if let Some(inv) = body.skill_invocation {
        skills.explicit = Some(crate::skills::ExplicitSkillInvocation {
            skill_id: inv.skill_id,
            version: inv.skill_version,
        });
    }

    let records = crate::work::enqueue_with_skills(
        &state.pool,
        principal.owner_id(),
        &request_id,
        body.bot_id.trim(),
        body.conversation_id.as_deref(),
        body.message.trim(),
        &skills,
    )
    .await?;
    let run = find_run_for_owner(&state.pool, principal.owner_id(), &records.run_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::NotFound)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(CreateRunResponse {
            run_id: records.run_id,
            request_id: records.request_id,
            conversation_id: records.conversation_id,
            computer_id: records.computer_id,
            model: records.model,
            status: run.status,
        }),
    ))
}

async fn create_legacy_run(
    state: AppState,
    request_id: String,
    body: CreateRunRequest,
) -> Result<(StatusCode, Json<CreateRunResponse>), ApiError> {
    if body.message.trim().is_empty() {
        return Err(ApiError::Validation("message cannot be empty".into()));
    }
    if body.bot.id.trim().is_empty() || body.bot.computer_id.trim().is_empty() {
        return Err(ApiError::Validation(
            "bot.id and bot.computerId are required".into(),
        ));
    }

    let existing_before = crate::db::queries::find_run_by_request_id(&state.pool, &request_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let permit = if existing_before.is_none() {
        match state.run_semaphore.clone().try_acquire_owned() {
            Ok(permit) => Some(permit),
            Err(_) => return Err(ApiError::TooManyRequests),
        }
    } else {
        None
    };

    let records = bootstrap_run_legacy(
        &state.pool,
        &request_id,
        &body.bot.id,
        &body.bot.name,
        &body.bot.instructions,
        body.bot.model.as_deref(),
        &body.bot.computer_id,
        body.conversation_id.as_deref(),
        body.message.trim(),
    )
    .await?;

    finish_create_run(state, records, body.bot.id, body.message, permit).await
}

async fn finish_create_run(
    state: AppState,
    records: crate::db::queries::BootstrapRunRecords,
    bot_id: String,
    user_message: String,
    permit: Option<tokio::sync::OwnedSemaphorePermit>,
) -> Result<(StatusCode, Json<CreateRunResponse>), ApiError> {
    let run_row = crate::db::queries::find_run_by_id(&state.pool, &records.run_id)
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

    if !records.is_new_run || run_row.status != "running" {
        if let Some(permit) = permit {
            drop(permit);
        }
        return Ok((StatusCode::ACCEPTED, Json(response)));
    }

    let permit = match permit {
        Some(permit) => permit,
        None => match state.run_semaphore.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => return Err(ApiError::TooManyRequests),
        },
    };

    if state.registry.get(&records.run_id).is_some() {
        drop(permit);
        return Ok((StatusCode::ACCEPTED, Json(response)));
    }

    spawn_agent_run(
        state.clone(),
        RunExecutionInput {
            records,
            bot_id,
            user_message: user_message.trim().to_string(),
            engine_mode: None,
        },
        permit,
    );

    Ok((StatusCode::ACCEPTED, Json(response)))
}

pub async fn get_run(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(run_id): Path<String>,
) -> Result<Json<RunDetailResponse>, ApiError> {
    let run = find_run_for_owner(&state.pool, principal.owner_id(), &run_id)
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

    let task: Option<String> =
        sqlx::query_scalar("SELECT user_message FROM work_queue WHERE run_id = $1")
            .bind(&run.id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(RunDetailResponse {
        task,
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

pub async fn archive_run(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(run_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    crate::run_archive::archive_run(&state, principal.owner_id(), &run_id)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn cancel_run(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(run_id): Path<String>,
) -> Result<(StatusCode, Json<RunDetailResponse>), ApiError> {
    let run = find_run_for_owner(&state.pool, principal.owner_id(), &run_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::NotFound)?;

    crate::work::request_cancel(&state.pool, principal.owner_id(), &run_id).await?;
    if run.status == "running" {
        state.registry.cancel(&run_id);
        let _ = state
            .approvals
            .cancel_pending_for_run(&run_id, "run_cancelled")
            .await;
        let _ = state
            .human_interventions
            .cancel_pending_for_run(&run_id, "run_cancelled")
            .await;
    }

    let updated = get_run(State(state), Extension(principal), Path(run_id)).await?;
    Ok((StatusCode::ACCEPTED, updated))
}

pub async fn run_events_sse(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(run_id): Path<String>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let run = find_run_for_owner(&state.pool, principal.owner_id(), &run_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::NotFound)?;

    let after_id = headers
        .get("Last-Event-ID")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    let pool = state.pool.clone();
    let registry = state.registry.clone();
    let request_id = run.request_id.clone();
    let stream = async_stream::stream! {
        let mut last_id = after_id;
        let mut live_rx = registry.subscribe_live(&run_id);

        loop {
            let durable = match list_run_events_after(&pool, &request_id, last_id, 500).await {
                Ok(rows) => rows,
                Err(_) => {
                    yield Ok(Event::default().event("stream_error").data("{\"detail\":\"Progress temporarily unavailable. Reconnecting is safe.\"}"));
                    break;
                }
            };
            let batch_full = durable.len() == 500;
            for row in durable {
                last_id = row.id;
                yield Ok(Event::default().id(row.id.to_string()).event(row.event_type).data(row.payload_json.to_string()));
            }
            if batch_full {
                continue;
            }

            let current = match crate::db::queries::find_run_by_id(&pool, &run_id).await {
                Ok(Some(run)) => run,
                _ => break,
            };
            if !matches!(current.status.as_str(), "queued" | "running") {
                match list_run_events_after(&pool, &request_id, last_id, 500).await {
                    Ok(rows) if !rows.is_empty() => { continue; }
                    Err(_) => break,
                    _ => {}
                }
                yield Ok(Event::default().event("terminal").data(serde_json::json!({"status":current.status,"errorCode":current.error_code}).to_string()));
                break;
            }

            if live_rx.is_none() {
                live_rx = registry.subscribe_live(&run_id);
            }

            if let Some(rx) = live_rx.as_mut() {
                match tokio::time::timeout(Duration::from_millis(750), rx.recv()).await {
                    Ok(Ok(live)) => {
                        if live.id > last_id {
                            last_id = live.id;
                            yield Ok(Event::default()
                                .id(live.id.to_string())
                                .event(live.event_type)
                                .data(live.payload.to_string()));
                        }
                    }
                    Ok(Err(broadcast::error::RecvError::Lagged(_))) => {
                        live_rx = None;
                    }
                    Ok(Err(_)) => {
                        live_rx = None;
                    }
                    Err(_) => {}
                }
            } else {
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
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
    #[test]
    fn default_model_is_luna_when_omitted() {
        assert_eq!(agent_core::DEFAULT_MODEL, "gpt-5.6-luna");
    }
}
