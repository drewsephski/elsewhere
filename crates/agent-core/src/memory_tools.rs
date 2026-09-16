use serde_json::{json, Value};

use crate::approval::{ToolApprovalContext, ToolApprovalGate, ToolRunContext};
use crate::memory::{
    AgentMemory, MemoryContext, MemoryError, FORGET_MEMORY_DESCRIPTION, FORGET_MEMORY_TOOL_NAME,
    MAX_MEMORY_CONTENT_BYTES, MAX_MEMORY_RECALL_LIMIT, RECALL_MEMORY_DESCRIPTION,
    RECALL_MEMORY_TOOL_NAME, REMEMBER_DESCRIPTION, REMEMBER_TOOL_NAME,
};
use crate::tool_catalog::is_memory_tool;
use crate::tools::ToolError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub fn memory_openai_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "name": RECALL_MEMORY_TOOL_NAME,
            "description": RECALL_MEMORY_DESCRIPTION,
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "What to look up in this Bot's memories"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum memories to return (1-8)"
                    }
                },
                "required": ["query"],
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": REMEMBER_TOOL_NAME,
            "description": REMEMBER_DESCRIPTION,
            "parameters": {
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "Concise standalone fact or preference to remember"
                    },
                    "kind": {
                        "type": "string",
                        "enum": ["preference", "fact", "project", "constraint", "workflow", "relationship", "other"],
                        "description": "Optional memory kind"
                    }
                },
                "required": ["content"],
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": FORGET_MEMORY_TOOL_NAME,
            "description": FORGET_MEMORY_DESCRIPTION,
            "parameters": {
                "type": "object",
                "properties": {
                    "memoryId": {
                        "type": "string",
                        "description": "Memory id from recall_memory"
                    }
                },
                "required": ["memoryId"],
                "additionalProperties": false
            },
            "strict": true
        }),
    ]
}

pub async fn dispatch_memory_tool(
    memory: Option<&Arc<dyn AgentMemory>>,
    name: &str,
    arguments: &str,
    cancel: &AtomicBool,
    gate: &dyn ToolApprovalGate,
    run: &ToolRunContext,
    memory_ctx: Option<&MemoryContext>,
) -> Result<Value, ToolError> {
    if !is_memory_tool(name) {
        return Err(ToolError::MalformedArguments(format!(
            "unknown tool: {name}"
        )));
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(ToolError::Cancelled);
    }
    let service = memory.ok_or_else(|| {
        ToolError::MalformedArguments("bot memory is not available in this run".into())
    })?;
    let ctx =
        memory_ctx.ok_or_else(|| ToolError::MalformedArguments("memory context missing".into()))?;

    let args: Value = serde_json::from_str(arguments)
        .map_err(|e| ToolError::MalformedArguments(format!("invalid JSON arguments: {e}")))?;

    let approval_ctx = ToolApprovalContext::for_tool(run, name, args.clone());
    let approval = gate
        .authorize(&approval_ctx)
        .await
        .map_err(map_approval_error)?;
    if let crate::approval::ApprovalDecision::Deny { reason } = approval {
        return Err(ToolError::Denied(reason));
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(ToolError::Cancelled);
    }

    let result = match name {
        RECALL_MEMORY_TOOL_NAME => {
            let query = required_str(&args, "query")?;
            let limit = args.get("limit").and_then(|v| v.as_i64());
            if let Some(limit) = limit {
                if limit < 1 || limit > MAX_MEMORY_RECALL_LIMIT {
                    return Err(ToolError::MalformedArguments(
                        "limit must be between 1 and 8".into(),
                    ));
                }
            }
            let items = service
                .recall(ctx, query, limit)
                .await
                .map_err(map_memory_error)?;
            Ok(json!({ "ok": true, "memories": items }))
        }
        REMEMBER_TOOL_NAME => {
            let content = required_str(&args, "content")?;
            if content.len() > MAX_MEMORY_CONTENT_BYTES {
                return Err(ToolError::MalformedArguments(format!(
                    "content must be at most {MAX_MEMORY_CONTENT_BYTES} bytes"
                )));
            }
            let kind = args.get("kind").and_then(|v| v.as_str());
            let written = service
                .remember(ctx, content, kind)
                .await
                .map_err(map_memory_error)?;
            Ok(json!({
                "ok": true,
                "id": written.id,
                "status": written.status
            }))
        }
        FORGET_MEMORY_TOOL_NAME => {
            let memory_id = required_str(&args, "memoryId")?;
            let written = service
                .forget(ctx, memory_id)
                .await
                .map_err(map_memory_error)?;
            Ok(json!({
                "ok": true,
                "id": written.id,
                "status": written.status
            }))
        }
        other => Err(ToolError::MalformedArguments(format!(
            "unknown tool: {other}"
        ))),
    }?;

    let mut envelope = result;
    if let Some(obj) = envelope.as_object_mut() {
        obj.insert("tool".into(), json!(name));
    }
    Ok(envelope)
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ToolError::MalformedArguments(format!("missing {key}")))
}

fn map_approval_error(err: crate::approval::ApprovalError) -> ToolError {
    match err {
        crate::approval::ApprovalError::Cancelled => ToolError::Cancelled,
        crate::approval::ApprovalError::Denied { reason } => ToolError::Denied(reason),
        crate::approval::ApprovalError::TimedOut => ToolError::Denied("Approval timed out".into()),
        crate::approval::ApprovalError::Internal(m) => ToolError::Denied(m),
    }
}

fn map_memory_error(err: MemoryError) -> ToolError {
    match err {
        MemoryError::NotFound => ToolError::MalformedArguments("Memory not found".into()),
        MemoryError::Validation(m) | MemoryError::Capacity(m) => ToolError::MalformedArguments(m),
        MemoryError::Internal(m) => ToolError::Denied(m),
    }
}
