use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::approval::{ToolApprovalContext, ToolApprovalGate, ToolRunContext};
use crate::computer::AgentComputer;
use crate::github_coding::{
    AgentGithubCoding, GITHUB_OPEN_REPOSITORY_TOOL, GITHUB_PUBLISH_PULL_REQUEST_TOOL,
    GITHUB_REVIEW_PUBLISH_TOOL, GithubCodingError, is_github_coding_mutation_tool,
};
use crate::tool_catalog::is_github_coding_tool;
use crate::tools::ToolError;

pub fn github_coding_openai_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "name": GITHUB_OPEN_REPOSITORY_TOOL,
            "description": "Open an authorized GitHub repository into this Bot's workspace for editing and tests. Validates access against the connected GitHub App before checkout.",
            "parameters": {
                "type": "object",
                "properties": {
                    "owner": { "type": "string" },
                    "repo": { "type": "string" },
                    "taskSlug": {
                        "type": "string",
                        "description": "Short slug for the working branch (elsewhere/<slug>). Letters, numbers, and dashes only."
                    }
                },
                "required": ["owner", "repo"],
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": GITHUB_REVIEW_PUBLISH_TOOL,
            "description": "Summarize local changes, checks you ran, and the publish preview before asking the owner to approve GitHub publish.",
            "parameters": {
                "type": "object",
                "properties": {
                    "checkCommands": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Shell commands already executed via workspace_exec in this run. Elsewhere verifies results from durable tool events; do not report exit codes yourself."
                    }
                },
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": GITHUB_PUBLISH_PULL_REQUEST_TOOL,
            "description": "After owner approval, push the working branch and open a pull request on GitHub. Requires a prior github_review_publish in this run.",
            "parameters": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "body": { "type": "string" },
                    "publishAnyway": {
                        "type": "boolean",
                        "description": "Allow publish when recorded checks failed (owner must still approve)."
                    }
                },
                "required": ["title", "body"],
                "additionalProperties": false
            },
            "strict": true
        }),
    ]
}

pub async fn dispatch_github_coding_tool(
    github_coding: Option<&Arc<dyn AgentGithubCoding>>,
    computer: &dyn AgentComputer,
    name: &str,
    arguments: &str,
    cancel: &AtomicBool,
    gate: &dyn ToolApprovalGate,
    run: &ToolRunContext,
) -> Result<Value, ToolError> {
    if !is_github_coding_tool(name) {
        return Err(ToolError::MalformedArguments(format!("unknown tool: {name}")));
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(ToolError::Cancelled);
    }

    let service = github_coding.ok_or_else(|| {
        ToolError::MalformedArguments(
            "GitHub coding tools require a connected GitHub account".into(),
        )
    })?;

    let args: Value = serde_json::from_str(arguments)
        .map_err(|e| ToolError::MalformedArguments(format!("invalid JSON arguments: {e}")))?;

    if is_github_coding_mutation_tool(name) {
        let approval_args = service
            .approval_arguments(&run.owner_id, &run.run_id, computer, name, &args)
            .await
            .map_err(map_github_coding_error)?;
        let approval_ctx = ToolApprovalContext::for_tool(run, name, approval_args);
        authorize(gate, &approval_ctx).await?;
        if cancel.load(Ordering::Relaxed) {
            return Err(ToolError::Cancelled);
        }
        service
            .confirm_publish_approval(&run.owner_id, &run.run_id, computer)
            .await
            .map_err(map_github_coding_error)?;
        if cancel.load(Ordering::Relaxed) {
            return Err(ToolError::Cancelled);
        }
    }

    let result = service
        .dispatch_tool(
            &run.owner_id,
            &run.run_id,
            &run.request_id,
            &run.computer_id,
            computer,
            name,
            &args,
        )
        .await
        .map_err(map_github_coding_error)?;

    Ok(with_tool_name(result, name))
}

async fn authorize(gate: &dyn ToolApprovalGate, ctx: &ToolApprovalContext) -> Result<(), ToolError> {
    use crate::approval::{ApprovalDecision, ApprovalError};
    match gate.authorize(ctx).await {
        Ok(ApprovalDecision::Allow) => Ok(()),
        Ok(ApprovalDecision::Deny { reason }) => Err(ToolError::Denied(reason)),
        Err(ApprovalError::Cancelled) => Err(ToolError::Cancelled),
        Err(ApprovalError::Denied { reason }) => Err(ToolError::Denied(reason)),
        Err(ApprovalError::TimedOut) => Err(ToolError::Denied("approval timed out".into())),
        Err(ApprovalError::Internal(message)) => Err(ToolError::MalformedArguments(message)),
    }
}

fn map_github_coding_error(err: GithubCodingError) -> ToolError {
    match err {
        GithubCodingError::NotConnected | GithubCodingError::ReconnectRequired => {
            ToolError::MalformedArguments(err.message())
        }
        GithubCodingError::Validation(message) => ToolError::MalformedArguments(message),
        GithubCodingError::NotFound => ToolError::MalformedArguments(err.message()),
        GithubCodingError::Provider(message) => ToolError::Denied(message),
        GithubCodingError::Internal(message) => ToolError::MalformedArguments(message),
    }
}

fn with_tool_name(value: Value, name: &str) -> Value {
    match value {
        Value::Object(mut map) => {
            map.entry("tool".to_string()).or_insert_with(|| json!(name));
            Value::Object(map)
        }
        other => json!({ "tool": name, "result": other }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github_coding::is_github_coding_mutation_tool;

    #[test]
    fn publish_is_only_mutation() {
        assert!(is_github_coding_mutation_tool(GITHUB_PUBLISH_PULL_REQUEST_TOOL));
        assert!(!is_github_coding_mutation_tool(GITHUB_OPEN_REPOSITORY_TOOL));
        assert!(!is_github_coding_mutation_tool(GITHUB_REVIEW_PUBLISH_TOOL));
    }
}
