use crate::commands::STREAM_EVENT;
use crate::db::Database;
use crate::error::AppError;
use crate::models::{Message, MessageRole, MessageStatus, StreamEventPayload};
use crate::openai::{
    create_response, extract_assistant_text, extract_function_calls, function_call_output_item,
    model_supports_responses_tools, ResponsesCreateRequest,
};
use crate::state::AppState;
use crate::vm::SharedVirtualMachineManager;
use crate::agent::tools::{dispatch_tool, openai_tool_definitions, ToolError, MAX_AGENT_TOOL_STEPS};
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};
use tracing::{error, info};

pub struct AgentRunContext {
    pub app: AppHandle,
    pub vm: SharedVirtualMachineManager,
    pub api_key: String,
    pub request_id: String,
    pub conversation_id: String,
    pub assistant_message_id: String,
    pub bot_id: String,
    pub model: String,
    pub instructions: String,
    pub cancel: std::sync::Arc<AtomicBool>,
}

pub async fn run_agent_chat(ctx: AgentRunContext, mut input: Vec<Value>) -> Result<(), AppError> {
    if !model_supports_responses_tools(&ctx.model) {
        return Err(AppError::ModelUnavailable(format!(
            "Model {} does not support OpenAI Responses function tools",
            ctx.model
        )));
    }

    let client = Client::new();
    let tools = Value::Array(openai_tool_definitions());
    let mut step_count: i64 = 0;

    emit_agent_status(&ctx, "running", None);

    loop {
        if ctx.cancel.load(Ordering::Relaxed) {
            finalize_cancelled(&ctx)?;
            return Ok(());
        }

        if step_count as usize >= MAX_AGENT_TOOL_STEPS {
            return fail_run(
                &ctx,
                "max_tool_steps_exceeded",
                "Maximum tool steps exceeded",
                step_count,
            );
        }

        let response = match create_response(
            &client,
            &ctx.api_key,
            ResponsesCreateRequest {
                model: ctx.model.clone(),
                instructions: if ctx.instructions.trim().is_empty() {
                    None
                } else {
                    Some(ctx.instructions.clone())
                },
                input: Value::Array(input.clone()),
                tools: tools.clone(),
                tool_choice: Some("auto".into()),
                stream: Some(false),
            },
        )
        .await
        {
            Ok(response) => response,
            Err(err) => {
                let code = match &err {
                    AppError::ModelUnavailable(_) => "model_unavailable",
                    AppError::RateLimited(_) => "responses_api_failure",
                    AppError::Cancelled => "cancelled",
                    _ => "responses_api_failure",
                };
                if matches!(err, AppError::Cancelled) {
                    finalize_cancelled(&ctx)?;
                    return Ok(());
                }
                fail_run(&ctx, code, &err.to_string(), step_count)?;
                return Ok(());
            }
        };

        input.extend(response.output.clone());

        let function_calls = extract_function_calls(&response.output);
        if function_calls.is_empty() {
            let text = extract_assistant_text(&response.output, response.output_text.as_deref());
            finalize_success(&ctx, &text, step_count)?;
            return Ok(());
        }

        for call in function_calls {
            let (name, call_id, arguments) = call;
            if ctx.cancel.load(Ordering::Relaxed) {
                finalize_cancelled(&ctx)?;
                return Ok(());
            }

            step_count += 1;
            update_run_steps(&ctx, step_count)?;

            let call_body = json!({
                "tool": name,
                "callId": call_id,
                "arguments": serde_json::from_str::<Value>(&arguments).unwrap_or(json!({}))
            });
            let call_message = persist_event(
                &ctx,
                "tool_call",
                &call_body.to_string(),
                MessageStatus::Complete,
            )?;
            emit_message(&ctx, &call_message, None);

            let tool_result = match dispatch_tool(&ctx.vm, &name, &arguments, &ctx.cancel) {
                Ok(value) => value,
                Err(err) if err == ToolError::Cancelled => {
                    finalize_cancelled(&ctx)?;
                    return Ok(());
                }
                Err(err)
                    if matches!(
                        err,
                        ToolError::VmNotProvisioned
                            | ToolError::VmBootFailed(_)
                            | ToolError::GuestUnavailable(_)
                    ) =>
                {
                    let result_body = json!({
                        "tool": name,
                        "callId": call_id,
                        "ok": false,
                        "errorCode": err.code(),
                        "error": err.message()
                    });
                    let _ = persist_event(
                        &ctx,
                        "tool_result",
                        &result_body.to_string(),
                        MessageStatus::Error,
                    );
                    return fail_run(&ctx, err.code(), &err.message(), step_count);
                }
                Err(err) => {
                    let output_string = json!({
                        "ok": false,
                        "errorCode": err.code(),
                        "error": err.message()
                    })
                    .to_string();
                    let result_body = json!({
                        "tool": name,
                        "callId": call_id,
                        "ok": false,
                        "errorCode": err.code(),
                        "error": err.message(),
                        "output": output_string
                    });
                    let result_message = persist_event(
                        &ctx,
                        "tool_result",
                        &result_body.to_string(),
                        MessageStatus::Complete,
                    )?;
                    emit_message(&ctx, &result_message, None);
                    input.push(function_call_output_item(&call_id, &output_string));
                    continue;
                }
            };

            let output_string = serde_json::to_string(&tool_result)
                .map_err(|e| AppError::Provider(e.to_string()))?;

            let result_body = json!({
                "tool": name,
                "callId": call_id,
                "ok": tool_result.get("ok").cloned().unwrap_or(json!(true)),
                "durationMs": tool_result.get("durationMs").cloned().unwrap_or(json!(0)),
                "output": output_string
            });
            let result_message = persist_event(
                &ctx,
                "tool_result",
                &result_body.to_string(),
                MessageStatus::Complete,
            )?;
            emit_message(&ctx, &result_message, None);

            input.push(function_call_output_item(&call_id, &output_string));
        }
    }
}

pub fn build_responses_input_from_messages(messages: &[Message]) -> Result<Vec<Value>, AppError> {
    let mut input = Vec::new();
    for message in messages {
        if matches!(
            message.status,
            MessageStatus::Error | MessageStatus::Cancelled | MessageStatus::Interrupted
        ) {
            continue;
        }
        match message.kind.as_str() {
            "text" => match message.role {
                MessageRole::User => input.push(json!({
                    "role": "user",
                    "content": message.body
                })),
                MessageRole::Assistant => {
                    if !message.body.trim().is_empty() {
                        input.push(json!({
                            "role": "assistant",
                            "content": message.body
                        }));
                    }
                }
                MessageRole::System => {}
            },
            "tool_call" => {
                let parsed: Value = serde_json::from_str(&message.body)
                    .map_err(|e| AppError::Validation(format!("invalid tool_call body: {e}")))?;
                let name = parsed.get("tool").and_then(|v| v.as_str()).unwrap_or("");
                let call_id = parsed.get("callId").and_then(|v| v.as_str()).unwrap_or("");
                let arguments = parsed
                    .get("arguments")
                    .cloned()
                    .unwrap_or(json!({}));
                input.push(json!({
                    "type": "function_call",
                    "name": name,
                    "call_id": call_id,
                    "arguments": serde_json::to_string(&arguments).unwrap_or_else(|_| "{}".into())
                }));
            }
            "tool_result" => {
                let parsed: Value = serde_json::from_str(&message.body)
                    .map_err(|e| AppError::Validation(format!("invalid tool_result body: {e}")))?;
                let call_id = parsed.get("callId").and_then(|v| v.as_str()).unwrap_or("");
                let output = parsed
                    .get("output")
                    .and_then(|v| v.as_str())
                    .unwrap_or("{}");
                input.push(function_call_output_item(call_id, output));
            }
            _ => {}
        }
    }
    Ok(input)
}

fn with_db<T>(
    ctx: &AgentRunContext,
    f: impl FnOnce(&Database) -> Result<T, AppError>,
) -> Result<T, AppError> {
    let state = ctx.app.state::<AppState>();
    let db = state.db.lock();
    f(&db)
}

fn persist_event(
    ctx: &AgentRunContext,
    kind: &str,
    body: &str,
    status: MessageStatus,
) -> Result<Message, AppError> {
    with_db(ctx, |db| {
        db.insert_structured_message(
            &ctx.conversation_id,
            MessageRole::Assistant,
            kind,
            body,
            status,
            Some(&ctx.model),
        )
    })
}

fn update_run_steps(ctx: &AgentRunContext, step_count: i64) -> Result<(), AppError> {
    with_db(ctx, |db| db.update_agent_run(&ctx.request_id, "running", None, step_count))
}

fn finalize_success(ctx: &AgentRunContext, text: &str, step_count: i64) -> Result<(), AppError> {
    with_db(ctx, |db| {
        db.update_message_body_and_status(
            &ctx.assistant_message_id,
            text,
            MessageStatus::Complete,
            None,
        )?;
        db.touch_conversation(&ctx.conversation_id)?;
        db.touch_bot(&ctx.bot_id)?;
        db.update_agent_run(&ctx.request_id, "completed", None, step_count)
    })?;
    emit_agent_status(ctx, "completed", None);
    emit_terminal(ctx, "done", None, Some(text.to_string()));
    info!(request_id = %ctx.request_id, "agent run completed");
    Ok(())
}

fn finalize_cancelled(ctx: &AgentRunContext) -> Result<(), AppError> {
    let partial = with_db(ctx, |db| {
        let message = db.get_message(&ctx.assistant_message_id)?;
        db.update_message_body_and_status(
            &ctx.assistant_message_id,
            &message.body,
            MessageStatus::Cancelled,
            None,
        )?;
        db.update_agent_run(&ctx.request_id, "cancelled", Some("cancelled"), 0)?;
        Ok(message.body)
    })?;
    emit_terminal(ctx, "cancelled", None, Some(partial));
    Ok(())
}

fn fail_run(
    ctx: &AgentRunContext,
    code: &str,
    message: &str,
    step_count: i64,
) -> Result<(), AppError> {
    with_db(ctx, |db| {
        db.update_message_body_and_status(
            &ctx.assistant_message_id,
            "",
            MessageStatus::Error,
            Some(message),
        )?;
        db.update_agent_run(&ctx.request_id, "failed", Some(code), step_count)
    })?;
    emit_agent_status(ctx, "failed", Some(message.to_string()));
    emit_terminal(ctx, "error", Some(message.to_string()), None);
    error!(request_id = %ctx.request_id, error = %message, "agent run failed");
    Ok(())
}

fn emit_message(ctx: &AgentRunContext, message: &Message, error: Option<String>) {
    let _ = ctx.app.emit(
        STREAM_EVENT,
        StreamEventPayload {
            request_id: ctx.request_id.clone(),
            event_type: "message".to_string(),
            conversation_id: ctx.conversation_id.clone(),
            assistant_message_id: ctx.assistant_message_id.clone(),
            delta: None,
            error,
            full_content: None,
            message: Some(message.clone()),
        },
    );
}

fn emit_agent_status(ctx: &AgentRunContext, status: &str, detail: Option<String>) {
    let body = json!({ "status": status, "detail": detail }).to_string();
    if let Ok(message) = persist_event(ctx, "agent_status", &body, MessageStatus::Complete) {
        emit_message(ctx, &message, None);
    }
}

fn emit_terminal(
    ctx: &AgentRunContext,
    event_type: &str,
    error: Option<String>,
    full_content: Option<String>,
) {
    let _ = ctx.app.emit(
        STREAM_EVENT,
        StreamEventPayload {
            request_id: ctx.request_id.clone(),
            event_type: event_type.to_string(),
            conversation_id: ctx.conversation_id.clone(),
            assistant_message_id: ctx.assistant_message_id.clone(),
            delta: None,
            error,
            full_content,
            message: None,
        },
    );
}
