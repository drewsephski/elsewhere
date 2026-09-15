use serde_json::{json, Value};

use crate::human_intervention::{
    is_human_intervention_tool, sanitize_human_intervention_message,
    validate_human_intervention_reason, AgentHumanIntervention, HumanInterventionContext,
    HumanInterventionError, HUMAN_INTERVENTION_REASONS,
};
use crate::approval::{ToolApprovalContext, ToolApprovalGate, ToolRunContext};
use crate::browser_recovery::BrowserRecoverySession;
use crate::tools::ToolError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub fn human_intervention_openai_tool_definitions() -> Vec<Value> {
    vec![json!({
        "type": "function",
        "name": "browser_request_human",
        "description": "Ask the owner to complete a human-only browser step (login, CAPTCHA, 2FA, passkey, credentials, consent) after bounded autonomous recovery fails or when escalation is immediate. Pauses bot browser work until the owner returns control. Never ask for passwords or OTP values in chat.",
        "parameters": {
            "type": "object",
            "properties": {
                "reason": {
                    "type": "string",
                    "enum": HUMAN_INTERVENTION_REASONS,
                    "description": "Why owner action is required"
                },
                "message": {
                    "type": "string",
                    "description": "Short user-facing explanation (no secrets or page dumps)"
                }
            },
            "required": ["reason", "message"],
            "additionalProperties": false
        },
        "strict": true
    })]
}

pub async fn dispatch_human_intervention_tool(
    service: Option<&Arc<dyn AgentHumanIntervention>>,
    name: &str,
    arguments: &str,
    cancel: &AtomicBool,
    gate: &dyn ToolApprovalGate,
    run: &ToolRunContext,
    browser_recovery: Option<&Arc<BrowserRecoverySession>>,
) -> Result<Value, ToolError> {
    if !is_human_intervention_tool(name) {
        return Err(ToolError::MalformedArguments(format!("unknown tool: {name}")));
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(ToolError::Cancelled);
    }

    let args: Value = serde_json::from_str(arguments).map_err(|e| {
        ToolError::MalformedArguments(format!("invalid JSON arguments: {e}"))
    })?;
    let reason = args
        .get("reason")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::MalformedArguments("missing reason".into()))?;
    let message = args
        .get("message")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::MalformedArguments("missing message".into()))?;

    validate_human_intervention_reason(reason).map_err(map_human_intervention_error)?;
    let safe_message = sanitize_human_intervention_message(message)
        .map_err(map_human_intervention_error)?;

    let approval_ctx = ToolApprovalContext::for_tool(run, name, json!({
        "reason": reason,
        "message": safe_message.clone(),
    }));
    let approval = gate.authorize(&approval_ctx).await.map_err(map_approval_error)?;
    if let crate::approval::ApprovalDecision::Deny { reason } = approval {
        return Err(ToolError::Denied(reason));
    }

    let backend = service.ok_or_else(|| {
        ToolError::MalformedArguments("human intervention is not available in this run".into())
    })?;

    if cancel.load(Ordering::Relaxed) {
        return Err(ToolError::Cancelled);
    }

    let ctx = HumanInterventionContext {
        owner_id: run.owner_id.clone(),
        run_id: run.run_id.clone(),
        request_id: run.request_id.clone(),
        computer_id: run.computer_id.clone(),
    };

    let outcome = backend
        .request_and_wait(&ctx, reason, &safe_message, cancel)
        .await
        .map_err(map_human_intervention_error)?;

    if let Some(session) = browser_recovery {
        session.mark_owner_handback();
    }

    Ok(json!({
        "ok": true,
        "interventionId": outcome.intervention_id,
        "status": "resolved",
        "detail": "The owner returned control. Call browser_snapshot before continuing browser work; reassess the page and do not assume your requested step succeeded."
    }))
}

fn map_human_intervention_error(err: HumanInterventionError) -> ToolError {
    match err {
        HumanInterventionError::Validation(m) => ToolError::MalformedArguments(m),
        HumanInterventionError::DuplicatePending => {
            ToolError::Denied("A human intervention is already pending for this run".into())
        }
        HumanInterventionError::Cancelled => ToolError::Cancelled,
        HumanInterventionError::Internal(m) => ToolError::MalformedArguments(m),
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
