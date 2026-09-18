use serde_json::{json, Value};

use crate::approval::{ToolApprovalContext, ToolApprovalGate, ToolRunContext};
use crate::tool_catalog::is_routine_tool;
use crate::routines::{
    AgentRoutines, BotRoutineSchedule, RoutineContext, RoutineError,
    ROUTINE_CREATE_TOOL_NAME, ROUTINE_CREATE_DESCRIPTION, ROUTINE_LIST_DESCRIPTION,
    ROUTINE_LIST_TOOL_NAME, ROUTINE_PAUSE_TOOL_NAME, ROUTINE_PAUSE_DESCRIPTION,
    ROUTINE_RESUME_TOOL_NAME, ROUTINE_RESUME_DESCRIPTION,
};
use crate::tools::ToolError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub fn routine_openai_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "name": ROUTINE_LIST_TOOL_NAME,
            "description": ROUTINE_LIST_DESCRIPTION,
            "parameters": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": ROUTINE_CREATE_TOOL_NAME,
            "description": ROUTINE_CREATE_DESCRIPTION,
            "parameters": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Short routine name the owner will recognize" },
                    "instructions": { "type": "string", "description": "What this Bot should do each time the routine runs" },
                    "schedule": {
                        "type": "object",
                        "properties": {
                            "repeat": {
                                "type": "string",
                                "enum": ["every_minutes", "daily", "weekdays", "weekly"],
                                "description": "every_minutes: interval; daily: every day at `at`; weekdays: Mon–Fri at `at`; weekly: selected `days` at `at`"
                            },
                            "everyMinutes": { "type": "integer", "description": "Minutes between runs when repeat is every_minutes (15–43200)" },
                            "at": { "type": "string", "description": "Local time HH:MM for daily, weekdays, or weekly" },
                            "days": { "type": "string", "description": "For weekly: weekdays, MON,TUE,..., or presets like MON,WED,FRI" }
                        },
                        "required": ["repeat"],
                        "additionalProperties": false
                    },
                    "timezone": { "type": "string", "description": "IANA timezone such as America/Chicago" },
                    "destinationConversationId": {
                        "type": "string",
                        "description": "Optional conversation id for routine output; defaults to this chat"
                    }
                },
                "required": ["name", "instructions", "schedule", "timezone"],
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": ROUTINE_PAUSE_TOOL_NAME,
            "description": ROUTINE_PAUSE_DESCRIPTION,
            "parameters": {
                "type": "object",
                "properties": {
                    "routineId": { "type": "string" }
                },
                "required": ["routineId"],
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": ROUTINE_RESUME_TOOL_NAME,
            "description": ROUTINE_RESUME_DESCRIPTION,
            "parameters": {
                "type": "object",
                "properties": {
                    "routineId": { "type": "string" }
                },
                "required": ["routineId"],
                "additionalProperties": false
            },
            "strict": true
        }),
    ]
}

pub async fn dispatch_routine_tool(
    routines: Option<&Arc<dyn AgentRoutines>>,
    name: &str,
    arguments: &str,
    cancel: &AtomicBool,
    gate: &dyn ToolApprovalGate,
    run: &ToolRunContext,
    routine_ctx: Option<&RoutineContext>,
) -> Result<Value, ToolError> {
    if !is_routine_tool(name) {
        return Err(ToolError::MalformedArguments(format!(
            "unknown tool: {name}"
        )));
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(ToolError::Cancelled);
    }
    let service = routines.ok_or_else(|| {
        ToolError::MalformedArguments("routine tools are not available in this run".into())
    })?;
    let ctx =
        routine_ctx.ok_or_else(|| ToolError::MalformedArguments("routine context missing".into()))?;

    let mut args: Value = serde_json::from_str(arguments)
        .map_err(|e| ToolError::MalformedArguments(format!("invalid JSON arguments: {e}")))?;

    if name == ROUTINE_CREATE_TOOL_NAME {
        let name = required_str(&args, "name")?;
        let instructions = required_str(&args, "instructions")?;
        let timezone = required_str(&args, "timezone")?;
        let schedule = parse_schedule(&args)?;
        let destination = args
            .get("destinationConversationId")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());
        let draft = service
            .validate_create(ctx, name, instructions, &schedule, timezone, destination)
            .await
            .map_err(map_routine_error)?;
        let approval_args = json!({
            "name": draft.name,
            "timezone": draft.timezone,
            "scheduleLabel": draft.schedule_label,
            "instructionsLength": draft.instructions.len(),
        });
        let approval_ctx =
            ToolApprovalContext::for_tool(run, ROUTINE_CREATE_TOOL_NAME, approval_args);
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
        let mutation = service
            .create_validated(ctx, &draft)
            .await
            .map_err(map_routine_error)?;
        return Ok(json!({
            "ok": true,
            "routine": mutation.routine,
            "message": mutation.message
        }));
    }

    if name == ROUTINE_PAUSE_TOOL_NAME || name == ROUTINE_RESUME_TOOL_NAME {
        let routine_id = args
            .get("routineId")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ToolError::MalformedArguments("missing routineId".into()))?;
        let rows = service.list(ctx).await.map_err(map_routine_error)?;
        let found = rows
            .iter()
            .find(|row| row.id == routine_id)
            .ok_or_else(|| ToolError::Denied("Routine not found for this Bot".into()))?;
        if let Some(obj) = args.as_object_mut() {
            obj.insert("routineName".into(), json!(found.name));
            obj.insert("scheduleLabel".into(), json!(found.schedule_label));
            obj.insert("timezone".into(), json!(found.timezone));
        }
    }

    if name != ROUTINE_LIST_TOOL_NAME {
        let approval_ctx = ToolApprovalContext::for_tool(run, name, args.clone());
        let approval = gate
            .authorize(&approval_ctx)
            .await
            .map_err(map_approval_error)?;
        if let crate::approval::ApprovalDecision::Deny { reason } = approval {
            return Err(ToolError::Denied(reason));
        }
    }

    if cancel.load(Ordering::Relaxed) {
        return Err(ToolError::Cancelled);
    }

    let result = match name {
        ROUTINE_LIST_TOOL_NAME => {
            let rows = service.list(ctx).await.map_err(map_routine_error)?;
            json!({ "ok": true, "routines": rows })
        }
        ROUTINE_PAUSE_TOOL_NAME => {
            let routine_id = required_str(&args, "routineId")?;
            let mutation = service
                .set_enabled(ctx, routine_id, false)
                .await
                .map_err(map_routine_error)?;
            json!({
                "ok": true,
                "routine": mutation.routine,
                "message": mutation.message
            })
        }
        ROUTINE_RESUME_TOOL_NAME => {
            let routine_id = required_str(&args, "routineId")?;
            let mutation = service
                .set_enabled(ctx, routine_id, true)
                .await
                .map_err(map_routine_error)?;
            json!({
                "ok": true,
                "routine": mutation.routine,
                "message": mutation.message
            })
        }
        other => {
            return Err(ToolError::MalformedArguments(format!(
                "unknown tool: {other}"
            )));
        }
    };

    Ok(result)
}

fn parse_schedule(args: &Value) -> Result<BotRoutineSchedule, ToolError> {
    let schedule = args
        .get("schedule")
        .ok_or_else(|| ToolError::MalformedArguments("missing schedule".into()))?;
    let repeat = schedule
        .get("repeat")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::MalformedArguments("schedule.repeat is required".into()))?;
    let every_minutes = schedule.get("everyMinutes").and_then(|v| v.as_i64()).map(|v| v as i32);
    let at = schedule
        .get("at")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let days = schedule
        .get("days")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    Ok(BotRoutineSchedule {
        repeat: repeat.to_string(),
        every_minutes,
        at,
        days,
    })
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ToolError::MalformedArguments(format!("missing or empty `{key}`")))
}

fn map_approval_error(err: crate::approval::ApprovalError) -> ToolError {
    match err {
        crate::approval::ApprovalError::Cancelled => ToolError::Cancelled,
        crate::approval::ApprovalError::Denied { reason } => ToolError::Denied(reason),
        crate::approval::ApprovalError::TimedOut => ToolError::Denied("approval timed out".into()),
        crate::approval::ApprovalError::Internal(detail) => ToolError::MalformedArguments(detail),
    }
}

fn map_routine_error(err: RoutineError) -> ToolError {
    match err {
        RoutineError::NotFound | RoutineError::Forbidden => {
            ToolError::Denied(err.message())
        }
        RoutineError::Validation(m) => ToolError::MalformedArguments(m),
        RoutineError::Internal(m) => ToolError::MalformedArguments(m),
    }
}
