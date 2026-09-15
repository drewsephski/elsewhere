use serde_json::{json, Value};

use crate::approval::{ToolApprovalContext, ToolApprovalGate, ToolRunContext};
use crate::connectors::{
    bound_connector_tool_result, AgentConnectors, ConnectorError,
};
use crate::tools::ToolError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub fn connector_openai_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "name": "github_list_repositories",
            "description": "List repositories visible to the connected GitHub account. Read-only.",
            "parameters": {
                "type": "object",
                "properties": {
                    "visibility": { "type": "string", "enum": ["all", "public", "private"] },
                    "perPage": { "type": "integer" },
                    "page": { "type": "integer" }
                },
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": "github_search_repositories",
            "description": "Search GitHub repositories. Read-only.",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "perPage": { "type": "integer" },
                    "page": { "type": "integer" }
                },
                "required": ["query"],
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": "github_get_repository",
            "description": "Get metadata for a GitHub repository. Read-only.",
            "parameters": {
                "type": "object",
                "properties": {
                    "owner": { "type": "string" },
                    "repo": { "type": "string" }
                },
                "required": ["owner", "repo"],
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": "github_get_file_contents",
            "description": "Read a file from a GitHub repository (text). Read-only.",
            "parameters": {
                "type": "object",
                "properties": {
                    "owner": { "type": "string" },
                    "repo": { "type": "string" },
                    "path": { "type": "string" },
                    "ref": { "type": "string" }
                },
                "required": ["owner", "repo", "path"],
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": "github_list_issues",
            "description": "List issues for a repository. Read-only.",
            "parameters": {
                "type": "object",
                "properties": {
                    "owner": { "type": "string" },
                    "repo": { "type": "string" },
                    "state": { "type": "string", "enum": ["open", "closed", "all"] },
                    "perPage": { "type": "integer" },
                    "page": { "type": "integer" }
                },
                "required": ["owner", "repo"],
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": "github_get_issue",
            "description": "Get a single issue by number. Read-only.",
            "parameters": {
                "type": "object",
                "properties": {
                    "owner": { "type": "string" },
                    "repo": { "type": "string" },
                    "number": { "type": "integer" }
                },
                "required": ["owner", "repo", "number"],
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": "github_list_pull_requests",
            "description": "List pull requests for a repository. Read-only.",
            "parameters": {
                "type": "object",
                "properties": {
                    "owner": { "type": "string" },
                    "repo": { "type": "string" },
                    "state": { "type": "string", "enum": ["open", "closed", "all"] },
                    "perPage": { "type": "integer" },
                    "page": { "type": "integer" }
                },
                "required": ["owner", "repo"],
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": "github_get_pull_request",
            "description": "Get a pull request by number. Read-only.",
            "parameters": {
                "type": "object",
                "properties": {
                    "owner": { "type": "string" },
                    "repo": { "type": "string" },
                    "number": { "type": "integer" }
                },
                "required": ["owner", "repo", "number"],
                "additionalProperties": false
            },
            "strict": true
        }),
    ]
}

pub async fn dispatch_connector_tool_with_gate(
    connectors: Option<&Arc<dyn AgentConnectors>>,
    name: &str,
    arguments: &str,
    cancel: &AtomicBool,
    gate: &dyn ToolApprovalGate,
    run: &ToolRunContext,
) -> Result<Value, ToolError> {
    if cancel.load(Ordering::Relaxed) {
        return Err(ToolError::Cancelled);
    }

    let service = connectors.ok_or_else(|| {
        ToolError::MalformedArguments("connectors are not available in this run".into())
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

    let result = service
        .dispatch_connector_tool(&run.owner_id, name, &args)
        .await
        .map_err(map_connector_error)?;
    let result = bound_connector_tool_result(result).map_err(map_connector_error)?;

    let mut envelope = result;
    if let Some(obj) = envelope.as_object_mut() {
        obj.insert("tool".into(), json!(name));
    }
    Ok(envelope)
}

fn map_connector_error(err: ConnectorError) -> ToolError {
    match err {
        ConnectorError::NotConnected => ToolError::Denied(err.message()),
        ConnectorError::Validation(m) => ToolError::MalformedArguments(m),
        ConnectorError::Provider(m) => ToolError::Denied(m),
        ConnectorError::Internal(m) => ToolError::MalformedArguments(m),
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
