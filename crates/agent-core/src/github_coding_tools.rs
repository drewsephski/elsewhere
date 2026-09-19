use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::approval::{ToolApprovalContext, ToolApprovalGate, ToolRunContext};
use crate::computer::AgentComputer;
use crate::approval::MAX_EXEC_COMMAND_CHARS;
use crate::github_coding::{
    AgentGithubCoding, GITHUB_OPEN_REPOSITORY_TOOL, GITHUB_PUBLISH_PULL_REQUEST_TOOL,
    GITHUB_REVIEW_PUBLISH_TOOL, GITHUB_RUN_CHECK_TOOL, GithubCodingError,
    is_github_coding_mutation_tool, is_github_coding_terminal_tool,
    requires_github_coding_owner_approval,
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
            "name": GITHUB_RUN_CHECK_TOOL,
            "description": "Run a shell check command in the repository workspace and certify the result against the current publishable source state. Use this instead of workspace_exec when the result should count toward github_review_publish.",
            "parameters": {
                "type": "object",
                "properties": {
                    "command": { "type": "string" }
                },
                "required": ["command"],
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": GITHUB_REVIEW_PUBLISH_TOOL,
            "description": "Summarize local changes and certified checks before asking the owner to approve GitHub publish.",
            "parameters": {
                "type": "object",
                "properties": {
                    "checkCommands": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Commands previously certified with github_run_check for the current workspace state."
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

    validate_github_coding_argument_limits(name, &args)?;

    if requires_github_coding_owner_approval(name) {
        let approval_args = service
            .approval_arguments(&run.owner_id, &run.run_id, computer, name, &args)
            .await
            .map_err(map_github_coding_error)?;
        let approval_ctx = ToolApprovalContext::for_tool(run, name, approval_args);
        authorize(gate, &approval_ctx).await?;
        if cancel.load(Ordering::Relaxed) {
            return Err(ToolError::Cancelled);
        }
        if is_github_coding_mutation_tool(name) {
            service
                .confirm_publish_approval(&run.owner_id, &run.run_id, computer)
                .await
                .map_err(map_github_coding_error)?;
            if cancel.load(Ordering::Relaxed) {
                return Err(ToolError::Cancelled);
            }
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

fn validate_github_coding_argument_limits(name: &str, args: &Value) -> Result<(), ToolError> {
    if name == GITHUB_RUN_CHECK_TOOL {
        let command = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
        if command.len() > MAX_EXEC_COMMAND_CHARS {
            return Err(ToolError::MalformedArguments(format!(
                "command exceeds {MAX_EXEC_COMMAND_CHARS} characters"
            )));
        }
    }
    Ok(())
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
    use crate::approval::{
        operation_kind_for_tool, AllowAllApprovalGate, ApprovalDecision, ToolApprovalContext,
        ToolOperationKind,
    };
    use crate::github_coding::{
        is_github_coding_mutation_tool, is_github_coding_terminal_tool,
        requires_github_coding_owner_approval,
    };
    use crate::tool_catalog::{ALL_AGENT_TOOL_NAMES, policy_action_group, PolicyActionGroup};
    use async_trait::async_trait;
    use std::sync::Arc;

    #[test]
    fn publish_is_only_github_coding_mutation() {
        assert!(is_github_coding_mutation_tool(GITHUB_PUBLISH_PULL_REQUEST_TOOL));
        assert!(!is_github_coding_mutation_tool(GITHUB_OPEN_REPOSITORY_TOOL));
        assert!(!is_github_coding_mutation_tool(GITHUB_REVIEW_PUBLISH_TOOL));
        assert!(!is_github_coding_mutation_tool(GITHUB_RUN_CHECK_TOOL));
    }

    #[test]
    fn run_check_is_terminal_tool_requiring_approval() {
        assert!(is_github_coding_terminal_tool(GITHUB_RUN_CHECK_TOOL));
        assert!(requires_github_coding_owner_approval(GITHUB_RUN_CHECK_TOOL));
        assert!(!is_github_coding_mutation_tool(GITHUB_RUN_CHECK_TOOL));
        assert_eq!(
            operation_kind_for_tool(GITHUB_RUN_CHECK_TOOL),
            ToolOperationKind::Mutation
        );
        assert_eq!(
            policy_action_group(GITHUB_RUN_CHECK_TOOL),
            Some(PolicyActionGroup::Terminal)
        );
        assert!(ALL_AGENT_TOOL_NAMES.contains(&GITHUB_RUN_CHECK_TOOL));
    }

    #[test]
    fn overlong_run_check_command_rejected() {
        let long = "a".repeat(MAX_EXEC_COMMAND_CHARS + 1);
        let err = validate_github_coding_argument_limits(
            GITHUB_RUN_CHECK_TOOL,
            &json!({ "command": long }),
        )
        .expect_err("long");
        assert!(err.message().contains("exceeds"));
    }

    struct RecordingGithubCoding {
        dispatched: std::sync::Mutex<bool>,
    }

    #[async_trait]
    impl AgentGithubCoding for RecordingGithubCoding {
        async fn dispatch_tool(
            &self,
            _owner_id: &str,
            _run_id: &str,
            _request_id: &str,
            _computer_id: &str,
            _computer: &dyn AgentComputer,
            tool_name: &str,
            _arguments: &Value,
        ) -> Result<Value, GithubCodingError> {
            assert_eq!(tool_name, GITHUB_RUN_CHECK_TOOL);
            *self.dispatched.lock().unwrap() = true;
            Ok(json!({ "ok": true }))
        }
    }

    struct DenyGate;

    #[async_trait]
    impl ToolApprovalGate for DenyGate {
        async fn authorize(
            &self,
            _context: &ToolApprovalContext,
        ) -> Result<ApprovalDecision, crate::approval::ApprovalError> {
            Ok(ApprovalDecision::Deny {
                reason: "terminal denied".into(),
            })
        }
    }

    #[tokio::test]
    async fn denied_terminal_policy_blocks_run_check() {
        let recording = Arc::new(RecordingGithubCoding {
            dispatched: std::sync::Mutex::new(false),
        });
        let service: Arc<dyn AgentGithubCoding> = recording.clone();
        let computer = crate::FakeAgentComputer::new();
        let cancel = AtomicBool::new(false);
        let run = ToolRunContext {
            run_id: "run".into(),
            request_id: "req".into(),
            owner_id: "owner".into(),
            bot_id: "bot".into(),
            computer_id: "comp".into(),
            tool_invocation_id: None,
        };
        let err = dispatch_github_coding_tool(
            Some(&service),
            &computer,
            GITHUB_RUN_CHECK_TOOL,
            r#"{"command":"true"}"#,
            &cancel,
            &DenyGate,
            &run,
        )
        .await
        .expect_err("denied");
        assert!(matches!(err, ToolError::Denied(_)));
        assert!(!*recording.dispatched.lock().unwrap());
    }

    #[tokio::test]
    async fn allowed_policy_executes_run_check() {
        let recording = Arc::new(RecordingGithubCoding {
            dispatched: std::sync::Mutex::new(false),
        });
        let service: Arc<dyn AgentGithubCoding> = recording.clone();
        let computer = crate::FakeAgentComputer::new();
        let cancel = AtomicBool::new(false);
        let run = ToolRunContext {
            run_id: "run".into(),
            request_id: "req".into(),
            owner_id: "owner".into(),
            bot_id: "bot".into(),
            computer_id: "comp".into(),
            tool_invocation_id: None,
        };
        dispatch_github_coding_tool(
            Some(&service),
            &computer,
            GITHUB_RUN_CHECK_TOOL,
            r#"{"command":"true"}"#,
            &cancel,
            &AllowAllApprovalGate,
            &run,
        )
        .await
        .expect("allowed");
        assert!(*recording.dispatched.lock().unwrap());
    }
}
