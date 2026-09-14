use serde_json::{json, Value};

use std::time::{Duration, Instant};

use agent_core::{
    AgentEvent, AgentLoopContext, EventSink, MessageRole, MessageStatus, RuntimeError, RunStore,
    SharedRunDeps, StructuredMessageInput,
};

use crate::assistant_stream::{phase_to_event_str, CoalescedAssistantDelta};
const CHECKPOINT_INTERVAL: Duration = Duration::from_secs(2);
const CHECKPOINT_BYTE_DELTA: usize = 512;

pub(crate) async fn persist_event(
    deps: &SharedRunDeps,
    ctx: &AgentLoopContext,
    kind: &str,
    body: &str,
    status: MessageStatus,
    run_event_type: Option<&str>,
    payload: &Value,
) -> Result<agent_core::PersistedMessage, RuntimeError> {
    if let Some(event_type) = run_event_type {
        let receipt = deps
            .store
            .append_run_event(&ctx.request_id, event_type, payload)
            .await?;
        deps.events
            .emit_durable(receipt.id, event_type, payload)?;
    }
    deps.store
        .persist_structured_message(StructuredMessageInput {
            conversation_id: ctx.conversation_id.clone(),
            role: MessageRole::Assistant,
            kind: kind.to_string(),
            body: body.to_string(),
            status,
            model: Some(ctx.model.clone()),
        })
        .await
}

pub(crate) async fn emit_status(
    deps: &SharedRunDeps,
    ctx: &AgentLoopContext,
    status: &str,
    detail: Option<String>,
) -> Result<(), RuntimeError> {
    let body = json!({ "status": status, "detail": detail }).to_string();
    let payload = json!({ "status": status, "detail": detail });
    let message = persist_event(
        deps,
        ctx,
        "agent_status",
        &body,
        MessageStatus::Complete,
        Some("status"),
        &payload,
    )
    .await?;
    deps.events.emit(AgentEvent::StatusChanged {
        status: status.to_string(),
        detail,
        message: Some(message),
    })?;
    Ok(())
}

pub(crate) async fn persist_assistant_delta(
    deps: &SharedRunDeps,
    ctx: &AgentLoopContext,
    chunk: &CoalescedAssistantDelta,
    phases: Option<&mut crate::run_phases::RunPhaseRecorder>,
) -> Result<(), RuntimeError> {
    if let Some(phases) = phases {
        phases.mark_first_assistant_delta();
    }
    let payload = json!({
        "itemId": chunk.item_id,
        "phase": phase_to_event_str(chunk.phase),
        "delta": chunk.delta,
        "startOffset": chunk.start_offset,
        "endOffset": chunk.end_offset,
        "cumulativeLength": chunk.end_offset,
    });
    let receipt = deps
        .store
        .append_run_event(&ctx.request_id, "assistant_delta", &payload)
        .await?;
    deps.events
        .emit_durable(receipt.id, "assistant_delta", &payload)?;
    Ok(())
}

#[derive(Clone)]
pub(crate) struct AssistantCheckpointState {
    pub last_at: Instant,
    pub last_len: usize,
    pub last_body: String,
}

impl Default for AssistantCheckpointState {
    fn default() -> Self {
        Self {
            last_at: Instant::now() - CHECKPOINT_INTERVAL,
            last_len: 0,
            last_body: String::new(),
        }
    }
}

pub(crate) async fn maybe_checkpoint_assistant_stream(
    deps: &SharedRunDeps,
    ctx: &AgentLoopContext,
    visible_body: &str,
    checkpoint: &mut AssistantCheckpointState,
) -> Result<(), RuntimeError> {
    if visible_body == checkpoint.last_body {
        return Ok(());
    }
    let now = Instant::now();
    let byte_growth = visible_body.len().saturating_sub(checkpoint.last_len);
    if now.duration_since(checkpoint.last_at) < CHECKPOINT_INTERVAL
        && byte_growth < CHECKPOINT_BYTE_DELTA
    {
        return Ok(());
    }
    deps.store
        .update_assistant_message(
            &ctx.assistant_message_id,
            visible_body,
            MessageStatus::Streaming,
            None,
        )
        .await?;
    checkpoint.last_at = now;
    checkpoint.last_len = visible_body.len();
    checkpoint.last_body = visible_body.to_string();
    Ok(())
}

pub(crate) async fn flush_assistant_stream(
    deps: &SharedRunDeps,
    ctx: &AgentLoopContext,
    chunks: &[CoalescedAssistantDelta],
    visible_body: &str,
    checkpoint: &mut AssistantCheckpointState,
    mut phases: Option<&mut crate::run_phases::RunPhaseRecorder>,
) -> Result<(), RuntimeError> {
    for chunk in chunks {
        persist_assistant_delta(deps, ctx, chunk, phases.as_deref_mut()).await?;
    }
    if !visible_body.is_empty() {
        maybe_checkpoint_assistant_stream(deps, ctx, visible_body, checkpoint).await?;
    }
    Ok(())
}

pub(crate) async fn emit_run_started(
    deps: &SharedRunDeps,
    ctx: &AgentLoopContext,
    provider_meta: &Value,
) -> Result<(), RuntimeError> {
    let started_payload = json!({
        "requestId": ctx.request_id,
        "engine": "codex_subscription",
        "model": ctx.model,
        "provider": provider_meta,
    });
    let started_receipt = deps
        .store
        .append_run_event(&ctx.request_id, "run_started", &started_payload)
        .await?;
    deps.events.emit_durable(
        started_receipt.id,
        "run_started",
        &started_payload,
    )?;
    deps.events.emit(AgentEvent::RunStarted {
        request_id: ctx.request_id.clone(),
    })?;
    emit_status(deps, ctx, "running", None).await?;
    Ok(())
}

pub(crate) async fn finalize_success(
    deps: &SharedRunDeps,
    ctx: &AgentLoopContext,
    text: &str,
    step_count: i64,
) -> Result<(), RuntimeError> {
    deps.store
        .update_assistant_message(
            &ctx.assistant_message_id,
            text,
            MessageStatus::Complete,
            None,
        )
        .await?;
    deps.store
        .touch_conversation_and_bot(&ctx.conversation_id, &ctx.bot_id)
        .await?;
    deps.store
        .update_run(&ctx.request_id, "completed", None, step_count)
        .await?;
    emit_status(deps, ctx, "completed", None).await?;
    deps.events.emit(AgentEvent::AssistantMessage {
        content: text.to_string(),
    })?;
    deps.events.emit(AgentEvent::RunCompleted { step_count })?;
    let terminal_payload = json!({
        "eventType": "done",
        "error": null,
        "fullContent": text,
        "stepCount": step_count,
    });
    let terminal_receipt = deps
        .store
        .append_run_event(&ctx.request_id, "terminal", &terminal_payload)
        .await?;
    deps.events
        .emit_durable(terminal_receipt.id, "terminal", &terminal_payload)?;
    deps.events.emit(AgentEvent::Terminal {
        event_type: "done".into(),
        error: None,
        full_content: Some(text.to_string()),
    })?;
    Ok(())
}

pub(crate) async fn finalize_cancelled(
    deps: &SharedRunDeps,
    ctx: &AgentLoopContext,
    partial: &str,
) -> Result<(), RuntimeError> {
    deps.store
        .update_assistant_message(
            &ctx.assistant_message_id,
            partial,
            MessageStatus::Cancelled,
            None,
        )
        .await?;
    deps.store
        .update_run(&ctx.request_id, "cancelled", Some("cancelled"), 0)
        .await?;
    deps.events.emit(AgentEvent::RunCancelled)?;
    let terminal_payload = json!({
        "eventType": "cancelled",
        "error": null,
        "fullContent": partial,
    });
    let terminal_receipt = deps
        .store
        .append_run_event(&ctx.request_id, "terminal", &terminal_payload)
        .await?;
    deps.events
        .emit_durable(terminal_receipt.id, "terminal", &terminal_payload)?;
    deps.events.emit(AgentEvent::Terminal {
        event_type: "cancelled".into(),
        error: None,
        full_content: Some(partial.to_string()),
    })?;
    Ok(())
}

pub(crate) async fn finalize_interrupted(
    deps: &SharedRunDeps,
    ctx: &AgentLoopContext,
    partial: &str,
    code: &str,
) -> Result<(), RuntimeError> {
    deps.store
        .update_assistant_message(
            &ctx.assistant_message_id,
            partial,
            MessageStatus::Interrupted,
            None,
        )
        .await?;
    deps.store
        .update_run(&ctx.request_id, "interrupted", Some(code), 0)
        .await?;
    emit_status(deps, ctx, "interrupted", Some(code.into())).await?;
    let terminal_payload = json!({
        "eventType": "interrupted",
        "error": code,
        "fullContent": partial,
    });
    let terminal_receipt = deps
        .store
        .append_run_event(&ctx.request_id, "terminal", &terminal_payload)
        .await?;
    deps.events
        .emit_durable(terminal_receipt.id, "terminal", &terminal_payload)?;
    deps.events.emit(AgentEvent::Terminal {
        event_type: "interrupted".into(),
        error: Some(code.to_string()),
        full_content: Some(partial.to_string()),
    })?;
    Ok(())
}

pub(crate) async fn fail_run(
    deps: &SharedRunDeps,
    ctx: &AgentLoopContext,
    code: &str,
    message: &str,
    partial: &str,
    step_count: i64,
) -> Result<(), RuntimeError> {
    deps.store
        .update_assistant_message(
            &ctx.assistant_message_id,
            partial,
            MessageStatus::Error,
            Some(message),
        )
        .await?;
    deps.store
        .update_run(&ctx.request_id, "failed", Some(code), step_count)
        .await?;
    emit_status(deps, ctx, "failed", Some(message.to_string())).await?;
    deps.events.emit(AgentEvent::RunFailed {
        code: code.to_string(),
        message: message.to_string(),
        step_count,
    })?;
    let terminal_payload = json!({
        "eventType": "error",
        "error": message,
        "fullContent": if partial.is_empty() { Value::Null } else { json!(partial) },
        "code": code,
        "stepCount": step_count,
    });
    let terminal_receipt = deps
        .store
        .append_run_event(&ctx.request_id, "terminal", &terminal_payload)
        .await?;
    deps.events
        .emit_durable(terminal_receipt.id, "terminal", &terminal_payload)?;
    deps.events.emit(AgentEvent::Terminal {
        event_type: "error".into(),
        error: Some(message.to_string()),
        full_content: if partial.is_empty() {
            None
        } else {
            Some(partial.to_string())
        },
    })?;
    Ok(())
}
