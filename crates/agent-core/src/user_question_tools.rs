use serde_json::{json, Value};

use crate::approval::{ToolApprovalContext, ToolApprovalGate, ToolRunContext};
use crate::tools::ToolError;
use crate::user_question::{
    validate_user_question, AgentUserQuestion, UserQuestionContext, UserQuestionError,
    ASK_USER_DESCRIPTION, ASK_USER_TOOL_NAME, MAX_ASK_USER_OPTIONS, MAX_ASK_USER_OPTION_CHARS,
    MAX_ASK_USER_QUESTION_CHARS, MIN_ASK_USER_OPTIONS,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub fn user_question_openai_tool_definitions() -> Vec<Value> {
    vec![json!({
        "type": "function",
        "name": ASK_USER_TOOL_NAME,
        "description": ASK_USER_DESCRIPTION,
        "parameters": {
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": format!("Short question, max {MAX_ASK_USER_QUESTION_CHARS} characters")
                },
                "options": {
                    "type": "array",
                    "minItems": MIN_ASK_USER_OPTIONS,
                    "maxItems": MAX_ASK_USER_OPTIONS,
                    "items": {
                        "type": "string",
                        "description": format!("Choice label, max {MAX_ASK_USER_OPTION_CHARS} characters")
                    }
                }
            },
            "required": ["question", "options"],
            "additionalProperties": false
        },
        "strict": true
    })]
}

pub async fn dispatch_user_question_tool(
    backend: Option<&Arc<dyn AgentUserQuestion>>,
    name: &str,
    arguments: &str,
    cancel: &AtomicBool,
    gate: &dyn ToolApprovalGate,
    run: &ToolRunContext,
) -> Result<Value, ToolError> {
    if name != ASK_USER_TOOL_NAME {
        return Err(ToolError::MalformedArguments(format!(
            "unknown tool: {name}"
        )));
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(ToolError::Cancelled);
    }

    let args: Value = serde_json::from_str(arguments)
        .map_err(|e| ToolError::MalformedArguments(format!("invalid JSON arguments: {e}")))?;
    let question = args
        .get("question")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::MalformedArguments("missing question".into()))?;
    let options = args
        .get("options")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ToolError::MalformedArguments("missing options".into()))?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_string)
                .ok_or_else(|| ToolError::MalformedArguments("options must be strings".into()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let request = validate_user_question(question, &options).map_err(map_user_question_error)?;

    let approval_ctx = ToolApprovalContext::for_tool(
        run,
        name,
        json!({
            "question": request.question,
            "options": request.options,
        }),
    );
    let approval = gate
        .authorize(&approval_ctx)
        .await
        .map_err(map_approval_error)?;
    if let crate::approval::ApprovalDecision::Deny { reason } = approval {
        return Err(ToolError::Denied(reason));
    }

    let backend = backend.ok_or_else(|| {
        ToolError::MalformedArguments("ask_user is not available in this run".into())
    })?;
    if cancel.load(Ordering::Relaxed) {
        return Err(ToolError::Cancelled);
    }

    let invocation_id = run
        .tool_invocation_id
        .clone()
        .unwrap_or_else(|| format!("ask-{}", run.run_id));
    let ctx = UserQuestionContext {
        owner_id: run.owner_id.clone(),
        run_id: run.run_id.clone(),
        request_id: run.request_id.clone(),
        tool_invocation_id: invocation_id,
    };
    let outcome = backend
        .ask_and_wait(&ctx, &request, cancel)
        .await
        .map_err(map_user_question_error)?;

    Ok(json!({
        "selectedIndex": outcome.selected_index,
        "selectedOption": outcome.selected_option,
        "questionId": outcome.question_id
    }))
}

fn map_user_question_error(err: UserQuestionError) -> ToolError {
    match err {
        UserQuestionError::Validation(m) => ToolError::MalformedArguments(m),
        UserQuestionError::DuplicatePending => {
            ToolError::Denied("A question is already waiting for the owner on this run".into())
        }
        UserQuestionError::LimitReached => ToolError::Denied(
            "This assignment already asked the maximum number of questions".into(),
        ),
        UserQuestionError::Cancelled | UserQuestionError::Interrupted => ToolError::Cancelled,
        UserQuestionError::Internal(m) => ToolError::MalformedArguments(m),
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
