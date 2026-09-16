use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::approval::ToolRunContext;
use crate::collaboration::CollaborationContext;
use crate::collaboration_tools::all_openai_tool_definitions;
use crate::computer::ComputerError;
use crate::events::{AgentEvent, EventSink, RuntimeError};
use crate::input::{MessageRole, MessageStatus};
use crate::model::{
    extract_assistant_text, extract_function_calls, function_call_output_item,
    model_supports_responses_tools, CreateResponseRequest, ModelError, ResponsesModel,
};
use crate::run_store::{RunStore, StructuredMessageInput};
use crate::tools::{ToolError, MAX_AGENT_TOOL_STEPS};

pub struct AgentLoopContext {
    pub request_id: String,
    pub conversation_id: String,
    pub assistant_message_id: String,
    pub bot_id: String,
    pub model: String,
    pub instructions: String,
}

pub struct AgentLoopDeps {
    pub computer: Arc<dyn crate::computer::AgentComputer>,
    pub store: Arc<dyn RunStore>,
    pub events: Arc<dyn EventSink>,
    pub model: Arc<dyn ResponsesModel>,
    pub cancel: Arc<AtomicBool>,
    pub approval_gate: Arc<dyn crate::approval::ToolApprovalGate>,
    pub run_id: String,
    pub owner_id: String,
    pub computer_id: String,
    pub collaboration: Option<Arc<dyn crate::collaboration::AgentCollaboration>>,
    pub connectors: Option<Arc<dyn crate::connectors::AgentConnectors>>,
    pub human_intervention: Option<Arc<dyn crate::human_intervention::AgentHumanIntervention>>,
    pub browser_recovery: Option<Arc<crate::browser_recovery::BrowserRecoverySession>>,
    pub subagents: Option<Arc<dyn crate::subagent::AgentSubagents>>,
    pub memory: Option<Arc<dyn crate::memory::AgentMemory>>,
}

pub async fn run_agent_loop(
    ctx: AgentLoopContext,
    deps: AgentLoopDeps,
    mut input: Vec<Value>,
) -> Result<(), RuntimeError> {
    if !model_supports_responses_tools(&ctx.model) {
        return Err(RuntimeError::Model(format!(
            "Model {} does not support OpenAI Responses function tools",
            ctx.model
        )));
    }

    let tools = Value::Array(all_openai_tool_definitions());
    let mut step_count: i64 = 0;

    let started_payload = serde_json::json!({ "requestId": ctx.request_id });
    let started_receipt = deps
        .store
        .append_run_event(&ctx.request_id, "run_started", &started_payload)
        .await?;
    deps.events
        .emit_durable(started_receipt.id, "run_started", &started_payload)?;
    deps.events.emit(AgentEvent::RunStarted {
        request_id: ctx.request_id.clone(),
    })?;
    emit_status(&deps, &ctx, "running", None).await?;

    loop {
        if deps.cancel.load(Ordering::Relaxed) {
            finalize_cancelled(&deps, &ctx).await?;
            return Ok(());
        }

        if step_count as usize >= MAX_AGENT_TOOL_STEPS {
            fail_run(
                &deps,
                &ctx,
                "max_tool_steps_exceeded",
                "Maximum tool steps exceeded",
                step_count,
            )
            .await?;
            return Ok(());
        }

        let response = match deps
            .model
            .create_response(CreateResponseRequest {
                model: ctx.model.clone(),
                instructions: if ctx.instructions.trim().is_empty() {
                    None
                } else {
                    Some(ctx.instructions.clone())
                },
                input: Value::Array(input.clone()),
                tools: tools.clone(),
                tool_choice: Some("auto".into()),
            })
            .await
        {
            Ok(response) => response,
            Err(err) => {
                if matches!(err, ModelError::Cancelled) {
                    finalize_cancelled(&deps, &ctx).await?;
                    return Ok(());
                }
                let code = match &err {
                    ModelError::Unavailable(_) => "model_unavailable",
                    ModelError::RateLimited(_) => "responses_api_failure",
                    _ => "responses_api_failure",
                };
                fail_run(&deps, &ctx, code, &err.to_string(), step_count).await?;
                return Ok(());
            }
        };

        input.extend(response.output.clone());

        let function_calls = extract_function_calls(&response.output);
        if function_calls.is_empty() {
            let text = extract_assistant_text(&response.output, response.output_text.as_deref());
            finalize_success(&deps, &ctx, &text, step_count).await?;
            return Ok(());
        }

        for (name, call_id, arguments) in function_calls {
            if deps.cancel.load(Ordering::Relaxed) {
                finalize_cancelled(&deps, &ctx).await?;
                return Ok(());
            }

            step_count += 1;
            deps.store
                .update_run(&ctx.request_id, "running", None, step_count)
                .await?;

            let call_body = json!({
                "tool": name,
                "callId": call_id,
                "arguments": serde_json::from_str::<Value>(&arguments).unwrap_or(json!({}))
            });

            let call_message = persist_event(
                &deps,
                &ctx,
                "tool_call",
                &call_body.to_string(),
                MessageStatus::Complete,
                Some("tool_call"),
                &call_body,
            )
            .await?;

            deps.events.emit(AgentEvent::ToolCall {
                tool: name.clone(),
                call_id: call_id.clone(),
                arguments: call_body.get("arguments").cloned().unwrap_or(json!({})),
                message: Some(call_message),
            })?;

            let tool_run = ToolRunContext {
                run_id: deps.run_id.clone(),
                request_id: ctx.request_id.clone(),
                owner_id: deps.owner_id.clone(),
                bot_id: ctx.bot_id.clone(),
                computer_id: deps.computer_id.clone(),
                tool_invocation_id: Some(call_id.clone()),
            };
            let collaboration_ctx = CollaborationContext {
                owner_id: deps.owner_id.clone(),
                source_bot_id: ctx.bot_id.clone(),
                source_run_id: deps.run_id.clone(),
                source_conversation_id: ctx.conversation_id.clone(),
                source_request_id: ctx.request_id.clone(),
                tool_invocation_id: call_id.clone(),
            };
            let tool_result =
                match crate::collaboration_tools::dispatch_agent_tool_with_gate_and_recovery(
                    deps.computer.as_ref(),
                    deps.collaboration.as_ref(),
                    deps.connectors.as_ref(),
                    deps.human_intervention.as_ref(),
                    deps.subagents.as_ref(),
                    deps.memory.as_ref(),
                    &name,
                    &arguments,
                    &deps.cancel,
                    deps.approval_gate.as_ref(),
                    &tool_run,
                    Some(&collaboration_ctx),
                    deps.browser_recovery.as_ref(),
                )
                .await
                {
                    Ok(value) => value,
                    Err(ToolError::Cancelled) => {
                        finalize_cancelled(&deps, &ctx).await?;
                        return Ok(());
                    }
                    Err(err) if matches!(err, ToolError::Denied(_)) => {
                        let result_body = json!({
                            "tool": name,
                            "callId": call_id,
                            "ok": false,
                            "errorCode": err.code(),
                            "error": err.message()
                        });
                        let _ = persist_event(
                            &deps,
                            &ctx,
                            "tool_result",
                            &result_body.to_string(),
                            MessageStatus::Error,
                            Some("tool_result"),
                            &result_body,
                        )
                        .await;
                        let output_string = json!({
                            "ok": false,
                            "errorCode": err.code(),
                            "error": err.message()
                        })
                        .to_string();
                        input.push(function_call_output_item(&call_id, &output_string));
                        continue;
                    }
                    Err(err) if is_computer_fatal(&err) => {
                        let result_body = json!({
                            "tool": name,
                            "callId": call_id,
                            "ok": false,
                            "errorCode": err.code(),
                            "error": err.message()
                        });
                        let _ = persist_event(
                            &deps,
                            &ctx,
                            "tool_result",
                            &result_body.to_string(),
                            MessageStatus::Error,
                            Some("tool_result"),
                            &result_body,
                        )
                        .await;
                        fail_run(&deps, &ctx, err.code(), &err.message(), step_count).await?;
                        return Ok(());
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
                            &deps,
                            &ctx,
                            "tool_result",
                            &result_body.to_string(),
                            MessageStatus::Complete,
                            Some("tool_result"),
                            &result_body,
                        )
                        .await?;
                        deps.events.emit(AgentEvent::ToolResult {
                            tool: name.clone(),
                            call_id: call_id.clone(),
                            ok: false,
                            output: output_string.clone(),
                            message: Some(result_message),
                        })?;
                        input.push(function_call_output_item(&call_id, &output_string));
                        continue;
                    }
                };

            let output_string = serde_json::to_string(&tool_result)
                .map_err(|e| RuntimeError::Validation(e.to_string()))?;

            let result_body = json!({
                "tool": name,
                "callId": call_id,
                "ok": tool_result.get("ok").cloned().unwrap_or(json!(true)),
                "durationMs": tool_result.get("durationMs").cloned().unwrap_or(json!(0)),
                "output": output_string
            });
            let result_message = persist_event(
                &deps,
                &ctx,
                "tool_result",
                &result_body.to_string(),
                MessageStatus::Complete,
                Some("tool_result"),
                &result_body,
            )
            .await?;

            deps.events.emit(AgentEvent::ToolResult {
                tool: name.clone(),
                call_id: call_id.clone(),
                ok: true,
                output: output_string.clone(),
                message: Some(result_message),
            })?;

            input.push(function_call_output_item(&call_id, &output_string));
        }
    }
}

fn is_computer_fatal(err: &ToolError) -> bool {
    matches!(
        err.computer_error(),
        Some(
            ComputerError::NotProvisioned
                | ComputerError::BootFailed(_)
                | ComputerError::GuestUnavailable(_),
        )
    )
}

async fn persist_event(
    deps: &AgentLoopDeps,
    ctx: &AgentLoopContext,
    kind: &str,
    body: &str,
    status: MessageStatus,
    run_event_type: Option<&str>,
    payload: &Value,
) -> Result<crate::run_store::PersistedMessage, RuntimeError> {
    if let Some(event_type) = run_event_type {
        let receipt = deps
            .store
            .append_run_event(&ctx.request_id, event_type, payload)
            .await?;
        deps.events.emit_durable(receipt.id, event_type, payload)?;
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

async fn emit_status(
    deps: &AgentLoopDeps,
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

async fn finalize_success(
    deps: &AgentLoopDeps,
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
    let terminal_payload = serde_json::json!({
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

async fn finalize_cancelled(
    deps: &AgentLoopDeps,
    ctx: &AgentLoopContext,
) -> Result<(), RuntimeError> {
    let partial = deps
        .store
        .get_assistant_message_body(&ctx.assistant_message_id)
        .await?;
    deps.store
        .update_assistant_message(
            &ctx.assistant_message_id,
            &partial,
            MessageStatus::Cancelled,
            None,
        )
        .await?;
    deps.store
        .update_run(&ctx.request_id, "cancelled", Some("cancelled"), 0)
        .await?;
    deps.events.emit(AgentEvent::RunCancelled)?;
    let terminal_payload = serde_json::json!({
        "eventType": "cancelled",
        "error": null,
        "fullContent": &partial,
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
        full_content: Some(partial),
    })?;
    Ok(())
}

async fn fail_run(
    deps: &AgentLoopDeps,
    ctx: &AgentLoopContext,
    code: &str,
    message: &str,
    step_count: i64,
) -> Result<(), RuntimeError> {
    deps.store
        .update_assistant_message(
            &ctx.assistant_message_id,
            "",
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
    let terminal_payload = serde_json::json!({
        "eventType": "error",
        "error": message,
        "fullContent": null,
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
        full_content: None,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::computer::{AgentComputer, ComputerInfo, ExecResult, WorkspaceEntry};
    use crate::events::AgentEvent;
    use crate::model::{CreateResponseResult, ResponsesModel};
    use crate::run_store::{CreateRunParams, PersistedMessage, RunStore};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct RecordingEvents {
        events: Mutex<Vec<String>>,
    }

    impl EventSink for RecordingEvents {
        fn emit(&self, event: AgentEvent) -> Result<(), RuntimeError> {
            let label = match event {
                AgentEvent::ToolCall { .. } => "tool_call",
                AgentEvent::ToolResult { .. } => "tool_result",
                AgentEvent::RunCompleted { .. } => "completed",
                AgentEvent::RunFailed { .. } => "failed",
                _ => "other",
            };
            self.events.lock().unwrap().push(label.to_string());
            Ok(())
        }
    }

    struct MemStore {
        runs: Mutex<HashMap<String, String>>,
    }

    #[async_trait]
    impl RunStore for MemStore {
        async fn create_run(&self, params: CreateRunParams) -> Result<String, RuntimeError> {
            self.runs
                .lock()
                .unwrap()
                .insert(params.request_id.clone(), "running".into());
            Ok("run-1".into())
        }

        async fn append_run_event(
            &self,
            _request_id: &str,
            _event_type: &str,
            _payload: &Value,
        ) -> Result<crate::run_store::RunEventReceipt, RuntimeError> {
            Ok(crate::run_store::RunEventReceipt { id: 1 })
        }

        async fn persist_structured_message(
            &self,
            input: StructuredMessageInput,
        ) -> Result<PersistedMessage, RuntimeError> {
            Ok(PersistedMessage {
                id: "msg-1".into(),
                conversation_id: input.conversation_id,
                kind: input.kind,
                body: input.body,
            })
        }

        async fn update_assistant_message(
            &self,
            _message_id: &str,
            _body: &str,
            _status: MessageStatus,
            _error_message: Option<&str>,
        ) -> Result<(), RuntimeError> {
            Ok(())
        }

        async fn update_run(
            &self,
            _request_id: &str,
            _status: &str,
            _error_code: Option<&str>,
            _step_count: i64,
        ) -> Result<(), RuntimeError> {
            Ok(())
        }

        async fn touch_conversation_and_bot(
            &self,
            _conversation_id: &str,
            _bot_id: &str,
        ) -> Result<(), RuntimeError> {
            Ok(())
        }

        async fn get_assistant_message_body(
            &self,
            _message_id: &str,
        ) -> Result<String, RuntimeError> {
            Ok(String::new())
        }
    }

    struct FakeComputer;

    #[async_trait]
    impl AgentComputer for FakeComputer {
        async fn ensure_ready(&self) -> Result<ComputerInfo, ComputerError> {
            Ok(ComputerInfo {
                ready: true,
                protocol_version: 1,
                detail: None,
            })
        }

        async fn list_dir(&self, _path: &str) -> Result<Vec<WorkspaceEntry>, ComputerError> {
            Ok(vec![])
        }

        async fn read_file(&self, _path: &str) -> Result<Vec<u8>, ComputerError> {
            Ok(vec![])
        }

        async fn write_file(&self, _path: &str, _data: &[u8]) -> Result<(), ComputerError> {
            Ok(())
        }

        async fn exec(&self, _command: &str) -> Result<ExecResult, ComputerError> {
            Ok(ExecResult {
                ok: true,
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 0,
            })
        }
    }

    struct ScriptedModel {
        steps: Mutex<Vec<CreateResponseResult>>,
    }

    #[async_trait]
    impl ResponsesModel for ScriptedModel {
        async fn create_response(
            &self,
            _request: CreateResponseRequest,
        ) -> Result<CreateResponseResult, ModelError> {
            let mut steps = self.steps.lock().unwrap();
            if steps.is_empty() {
                return Ok(CreateResponseResult {
                    output: vec![json!({
                        "type": "message",
                        "content": [{"type": "output_text", "text": "done"}]
                    })],
                    output_text: Some("done".into()),
                });
            }
            Ok(steps.remove(0))
        }
    }

    #[tokio::test]
    async fn tool_loop_emits_ordered_events() {
        let model = ScriptedModel {
            steps: Mutex::new(vec![
                CreateResponseResult {
                    output: vec![json!({
                        "type": "function_call",
                        "name": "workspace_read",
                        "call_id": "c1",
                        "arguments": "{\"path\":\"/workspace/a\"}"
                    })],
                    output_text: None,
                },
                CreateResponseResult {
                    output: vec![json!({
                        "type": "message",
                        "content": [{"type": "output_text", "text": "read ok"}]
                    })],
                    output_text: Some("read ok".into()),
                },
            ]),
        };

        let events = Arc::new(RecordingEvents {
            events: Mutex::new(vec![]),
        });

        let deps = AgentLoopDeps {
            computer: Arc::new(FakeComputer),
            store: Arc::new(MemStore {
                runs: Mutex::new(HashMap::new()),
            }),
            events: events.clone(),
            model: Arc::new(model),
            cancel: Arc::new(AtomicBool::new(false)),
            approval_gate: Arc::new(crate::approval::AllowAllApprovalGate),
            run_id: "run-1".into(),
            owner_id: "owner".into(),
            computer_id: "comp".into(),
            collaboration: None,
            connectors: None,
            human_intervention: None,
            browser_recovery: None,
            subagents: None,
            memory: None,
        };

        let ctx = AgentLoopContext {
            request_id: "req-1".into(),
            conversation_id: "conv-1".into(),
            assistant_message_id: "asst-1".into(),
            bot_id: "bot-1".into(),
            model: "gpt-5.6-luna".into(),
            instructions: String::new(),
        };

        run_agent_loop(
            ctx,
            deps,
            vec![json!({"role": "user", "content": "read file"})],
        )
        .await
        .expect("loop");

        let labels = events.events.lock().unwrap().clone();
        assert!(labels.iter().any(|l| l == "tool_call"));
        assert!(labels.iter().any(|l| l == "tool_result"));
        assert!(labels.iter().any(|l| l == "completed"));
    }
}
