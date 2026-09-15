use serde_json::{json, Value};

use crate::approval::{ToolApprovalContext, ToolApprovalGate, ToolRunContext};
use crate::collaboration::{AgentCollaboration, CollaborationContext, CollaborationError};
use crate::tool_catalog::is_collaboration_tool;
use crate::tools::ToolError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub fn collaboration_openai_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "name": "bot_list",
            "description": "List other Bots owned by the same user that you may hand work to asynchronously. Does not wait for them to finish.",
            "parameters": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": "bot_delegate",
            "description": "Hand a piece of work to another Bot asynchronously. This only queues durable work for the recipient; it does NOT run their task or wait for completion. Tell the user what you delegated and to whom; do not claim the other Bot already finished.",
            "parameters": {
                "type": "object",
                "properties": {
                    "targetBotId": { "type": "string", "description": "Recipient Bot id from bot_list" },
                    "instruction": { "type": "string", "description": "Clear task for the recipient to own" },
                    "context": { "type": "string", "description": "Optional bounded text context (not filesystem paths from your computer)" },
                    "onComplete": {
                        "type": "string",
                        "enum": ["resume_source", "none"],
                        "description": "resume_source: queue a follow-up on the source Bot when the recipient finishes; none: hand off without source follow-up"
                    }
                },
                "required": ["targetBotId", "instruction"],
                "additionalProperties": false
            },
            "strict": true
        }),
    ]
}

pub fn all_openai_tool_definitions() -> Vec<Value> {
    let mut tools = crate::tools::openai_tool_definitions();
    tools.extend(collaboration_openai_tool_definitions());
    tools
}

pub async fn dispatch_agent_tool_with_gate(
    computer: &dyn crate::computer::AgentComputer,
    collaboration: Option<&Arc<dyn AgentCollaboration>>,
    name: &str,
    arguments: &str,
    cancel: &AtomicBool,
    gate: &dyn ToolApprovalGate,
    run: &ToolRunContext,
    collaboration_ctx: Option<&CollaborationContext>,
) -> Result<Value, ToolError> {
    if is_collaboration_tool(name) {
        return dispatch_collaboration_tool(collaboration, name, arguments, cancel, gate, run, collaboration_ctx).await;
    }
    crate::tools::dispatch_tool_with_gate(computer, name, arguments, cancel, gate, run).await
}

async fn dispatch_collaboration_tool(
    collaboration: Option<&Arc<dyn AgentCollaboration>>,
    name: &str,
    arguments: &str,
    cancel: &AtomicBool,
    gate: &dyn ToolApprovalGate,
    run: &ToolRunContext,
    collaboration_ctx: Option<&CollaborationContext>,
) -> Result<Value, ToolError> {
    if cancel.load(Ordering::Relaxed) {
        return Err(ToolError::Cancelled);
    }
    let service = collaboration.ok_or_else(|| {
        ToolError::MalformedArguments("bot collaboration is not available in this run".into())
    })?;
    let ctx = collaboration_ctx.ok_or_else(|| {
        ToolError::MalformedArguments("collaboration context missing".into())
    })?;

    let args: Value = serde_json::from_str(arguments).map_err(|e| {
        ToolError::MalformedArguments(format!("invalid JSON arguments: {e}"))
    })?;

    let approval_ctx = ToolApprovalContext::for_tool(run, name, args.clone());
    let approval = gate.authorize(&approval_ctx).await.map_err(map_approval_error)?;
    if let crate::approval::ApprovalDecision::Deny { reason } = approval {
        return Err(ToolError::Denied(reason));
    }

    if cancel.load(Ordering::Relaxed) {
        return Err(ToolError::Cancelled);
    }

    let result = match name {
        "bot_list" => {
            let bots = service.list_bots(ctx).await.map_err(map_collaboration_error)?;
            Ok(json!({ "ok": true, "bots": bots }))
        }
        "bot_delegate" => {
            let target_bot_id = required_str(&args, "targetBotId")?;
            let instruction = required_str(&args, "instruction")?;
            let context = args
                .get("context")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty());
            let return_policy = args
                .get("onComplete")
                .and_then(|v| v.as_str())
                .unwrap_or("resume_source");
            if return_policy != "resume_source" && return_policy != "none" {
                return Err(ToolError::MalformedArguments(
                    "onComplete must be resume_source or none".into(),
                ));
            }
            let enqueued = service
                .delegate(ctx, target_bot_id, instruction, context, return_policy)
                .await
                .map_err(map_collaboration_error)?;
            Ok(json!({
                "ok": true,
                "delegationId": enqueued.delegation_id,
                "targetBotId": enqueued.target_bot_id,
                "targetBotName": enqueued.target_bot_name,
                "targetRunId": enqueued.target_run_id,
                "status": enqueued.status,
                "async": true,
                "detail": "Work queued for the recipient Bot. They have not finished yet."
            }))
        }
        other => Err(ToolError::MalformedArguments(format!("unknown tool: {other}"))),
    }?;

    let mut envelope = result;
    if let Some(obj) = envelope.as_object_mut() {
        obj.insert("tool".into(), json!(name));
    }
    Ok(envelope)
}

fn map_collaboration_error(err: CollaborationError) -> ToolError {
    match err {
        CollaborationError::NotFound => ToolError::Denied("Bot not found".into()),
        CollaborationError::Validation(m) => ToolError::MalformedArguments(m),
        CollaborationError::LimitExceeded(m) => ToolError::Denied(m),
        CollaborationError::Conflict(m) => ToolError::MalformedArguments(m),
        CollaborationError::Internal(m) => ToolError::MalformedArguments(m),
    }
}

fn map_approval_error(err: crate::approval::ApprovalError) -> ToolError {
    match err {
        crate::approval::ApprovalError::Cancelled => ToolError::Cancelled,
        crate::approval::ApprovalError::Denied { reason } => ToolError::Denied(reason),
        crate::approval::ApprovalError::TimedOut => ToolError::Denied("approval timed out".into()),
        crate::approval::ApprovalError::Internal(detail) => ToolError::MalformedArguments(detail),
    }
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ToolError::MalformedArguments(format!("missing or empty `{key}`")))
}
