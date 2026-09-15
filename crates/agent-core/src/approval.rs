use async_trait::async_trait;
use serde_json::{json, Value};

pub const MAX_EXEC_COMMAND_CHARS: usize = 500;
pub const MAX_WRITE_CONTENT_PREVIEW_CHARS: usize = 200;
pub const MAX_BROWSER_URL_CHARS: usize = 2048;

use crate::human_intervention::is_human_intervention_tool;
use crate::tool_catalog::{is_browser_tool, is_collaboration_tool, is_connector_tool};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolOperationKind {
    Read,
    Mutation,
}

#[derive(Debug, Clone)]
pub struct ToolRunContext {
    pub run_id: String,
    pub request_id: String,
    pub owner_id: String,
    pub bot_id: String,
    pub computer_id: String,
    /// Stable per tool invocation (Responses call id or MCP JSON-RPC request id).
    pub tool_invocation_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ToolApprovalContext {
    pub run_id: String,
    pub request_id: String,
    pub owner_id: String,
    pub bot_id: String,
    pub computer_id: String,
    pub tool_name: String,
    pub operation_kind: ToolOperationKind,
    pub arguments: Value,
}

impl ToolApprovalContext {
    pub fn for_tool(run: &ToolRunContext, tool_name: &str, arguments: Value) -> Self {
        Self {
            run_id: run.run_id.clone(),
            request_id: run.request_id.clone(),
            owner_id: run.owner_id.clone(),
            bot_id: run.bot_id.clone(),
            computer_id: run.computer_id.clone(),
            tool_name: tool_name.to_string(),
            operation_kind: operation_kind_for_tool(tool_name),
            arguments: sanitize_tool_arguments(tool_name, &arguments),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    Allow,
    Deny { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalError {
    Cancelled,
    Denied { reason: String },
    TimedOut,
    Internal(String),
}

#[async_trait]
pub trait ToolApprovalGate: Send + Sync {
    async fn authorize(
        &self,
        context: &ToolApprovalContext,
    ) -> Result<ApprovalDecision, ApprovalError>;
}

pub struct AllowAllApprovalGate;

#[async_trait]
impl ToolApprovalGate for AllowAllApprovalGate {
    async fn authorize(
        &self,
        _context: &ToolApprovalContext,
    ) -> Result<ApprovalDecision, ApprovalError> {
        Ok(ApprovalDecision::Allow)
    }
}

pub fn operation_kind_for_tool(tool_name: &str) -> ToolOperationKind {
    match tool_name {
        "bot_list" => ToolOperationKind::Read,
        "bot_delegate" => ToolOperationKind::Mutation,
        "browser_request_human" => ToolOperationKind::Read,
        name if is_connector_tool(name) => ToolOperationKind::Read,
        "workspace_list" | "workspace_read" => ToolOperationKind::Read,
        "browser_snapshot" => ToolOperationKind::Read,
        "workspace_write" | "workspace_exec" => ToolOperationKind::Mutation,
        name if is_browser_tool(name) && name != "browser_snapshot" => ToolOperationKind::Mutation,
        _ => ToolOperationKind::Mutation,
    }
}

/// Persist-safe tool argument snapshot for approval audit rows.
pub fn sanitize_tool_arguments(tool_name: &str, args: &Value) -> Value {
    match tool_name {
        "workspace_write" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let preview = if content.is_empty() {
                None
            } else if content.len() > MAX_WRITE_CONTENT_PREVIEW_CHARS {
                Some(content[..MAX_WRITE_CONTENT_PREVIEW_CHARS].to_string())
            } else {
                Some(content.to_string())
            };
            json!({
                "path": path,
                "contentLength": content.len(),
                "contentPreview": preview
            })
        }
        "workspace_exec" => {
            let command = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
            let command = truncate_str(command, MAX_EXEC_COMMAND_CHARS);
            json!({ "command": command })
        }
        "workspace_list" | "workspace_read" => json!({
            "path": args.get("path").and_then(|v| v.as_str()).unwrap_or("")
        }),
        "browser_navigate" | "browser_download" => {
            let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("");
            let url = truncate_str(url, MAX_BROWSER_URL_CHARS);
            json!({ "url": url })
        }
        "browser_click" | "browser_type" => json!({
            "ref": args.get("ref").and_then(|v| v.as_str()).unwrap_or(""),
            "textPreview": args
                .get("text")
                .and_then(|v| v.as_str())
                .map(|t| truncate_str(t, 80))
        }),
        "browser_screenshot" => json!({
            "path": args.get("path").and_then(|v| v.as_str()).unwrap_or("")
        }),
        "browser_snapshot" => json!({}),
        "browser_request_human" => json!({
            "reason": args.get("reason").and_then(|v| v.as_str()).unwrap_or(""),
            "messageLength": args.get("message").and_then(|v| v.as_str()).map(|s| s.len()).unwrap_or(0)
        }),
        "bot_list" => json!({}),
        "bot_delegate" => json!({
            "targetBotId": args.get("targetBotId").and_then(|v| v.as_str()).unwrap_or(""),
            "instructionLength": args.get("instruction").and_then(|v| v.as_str()).map(|s| s.len()).unwrap_or(0),
            "contextLength": args.get("context").and_then(|v| v.as_str()).map(|s| s.len()).unwrap_or(0),
            "onComplete": args.get("onComplete").and_then(|v| v.as_str()).unwrap_or("resume_source")
        }),
        _ if is_collaboration_tool(tool_name) => json!({}),
        _ => json!({}),
    }
}

pub fn approval_action_summary(tool_name: &str, sanitized: &Value) -> String {
    match tool_name {
        "workspace_write" => {
            let path = sanitized
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("/workspace");
            format!("Write {path}")
        }
        "workspace_exec" => {
            let command = sanitized
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            format!("Run: {command}")
        }
        "browser_navigate" => {
            let url = sanitized.get("url").and_then(|v| v.as_str()).unwrap_or("");
            format!("Browse: {url}")
        }
        "browser_click" => {
            let ref_id = sanitized.get("ref").and_then(|v| v.as_str()).unwrap_or("");
            format!("Click {ref_id}")
        }
        "browser_type" => {
            let ref_id = sanitized.get("ref").and_then(|v| v.as_str()).unwrap_or("");
            format!("Type into {ref_id}")
        }
        "browser_download" => {
            let url = sanitized.get("url").and_then(|v| v.as_str()).unwrap_or("");
            format!("Download: {url}")
        }
        "browser_screenshot" => {
            let path = sanitized
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("/workspace");
            format!("Screenshot {path}")
        }
        "browser_snapshot" => "Inspect web page".into(),
        other => format!("Approve {other}"),
    }
}

fn truncate_str(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn read_tools_classified_as_read() {
        assert_eq!(
            operation_kind_for_tool("github_list_repositories"),
            ToolOperationKind::Read
        );
        assert_eq!(
            operation_kind_for_tool("workspace_list"),
            ToolOperationKind::Read
        );
        assert_eq!(
            operation_kind_for_tool("browser_snapshot"),
            ToolOperationKind::Read
        );
        assert_eq!(
            operation_kind_for_tool("browser_screenshot"),
            ToolOperationKind::Mutation
        );
    }

    #[test]
    fn mutation_tools_classified() {
        assert_eq!(
            operation_kind_for_tool("workspace_write"),
            ToolOperationKind::Mutation
        );
        assert_eq!(
            operation_kind_for_tool("browser_navigate"),
            ToolOperationKind::Mutation
        );
        assert_eq!(
            operation_kind_for_tool("unknown_tool"),
            ToolOperationKind::Mutation
        );
    }

    #[tokio::test]
    async fn allow_all_gate_permits() {
        let gate = AllowAllApprovalGate;
        let decision = gate
            .authorize(&ToolApprovalContext {
                run_id: "run".into(),
                request_id: "req".into(),
                owner_id: "owner".into(),
                bot_id: "bot".into(),
                computer_id: "comp".into(),
                tool_name: "workspace_exec".into(),
                operation_kind: ToolOperationKind::Mutation,
                arguments: json!({"command":"echo hi"}),
            })
            .await
            .expect("authorize");
        assert_eq!(decision, ApprovalDecision::Allow);
    }

    #[test]
    fn truncate_str_does_not_split_multibyte_characters() {
        let text = format!("{}🎉", "a".repeat(79));
        assert!(text.len() > 80);
        assert_eq!(truncate_str(&text, 80), "a".repeat(79));

        let sanitized = sanitize_tool_arguments(
            "browser_type",
            &json!({"ref":"e1","text": text}),
        );
        assert_eq!(
            sanitized.get("textPreview"),
            Some(&json!("a".repeat(79)))
        );
    }

    #[test]
    fn sanitize_write_strips_content() {
        let sanitized = sanitize_tool_arguments(
            "workspace_write",
            &json!({"path":"/workspace/a.txt","content":"hello"}),
        );
        assert_eq!(sanitized.get("contentLength"), Some(&json!(5)));
        assert_eq!(
            sanitized.get("contentPreview"),
            Some(&json!("hello"))
        );
    }
}
