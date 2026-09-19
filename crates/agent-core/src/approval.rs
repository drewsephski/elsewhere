use async_trait::async_trait;
use serde_json::{json, Value};

pub const MAX_EXEC_COMMAND_CHARS: usize = 500;
pub const MAX_WRITE_CONTENT_PREVIEW_CHARS: usize = 200;
pub const MAX_BROWSER_URL_CHARS: usize = 2048;

use crate::connectors::ConnectorToolDefinition;
use crate::tool_catalog::{
    is_attachment_tool, is_browser_tool, is_collaboration_tool, is_connected_apps_execute_tool,
    is_connected_apps_tool, is_github_coding_tool, is_github_connector_tool, is_routine_tool,
    is_skill_tool, is_user_question_tool,
};
use crate::github_coding::is_github_coding_mutation_tool;
use crate::routines::is_routine_mutation_tool;
use crate::skills::is_skill_mutation_tool;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedAppApprovalInfo {
    pub install_id: String,
    pub app_name: String,
    pub remote_tool: String,
    pub read_only: bool,
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
    pub connected_app: Option<ConnectedAppApprovalInfo>,
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
            connected_app: None,
        }
    }

    pub fn for_connected_app_execute(
        run: &ToolRunContext,
        tool: &ConnectorToolDefinition,
        call_args: &Value,
    ) -> Self {
        let info = ConnectedAppApprovalInfo {
            install_id: tool.install_id.clone(),
            app_name: tool.source.clone(),
            remote_tool: tool.name.clone(),
            read_only: tool.read_only,
        };
        let operation_kind = if tool.read_only {
            ToolOperationKind::Read
        } else {
            ToolOperationKind::Mutation
        };
        Self {
            run_id: run.run_id.clone(),
            request_id: run.request_id.clone(),
            owner_id: run.owner_id.clone(),
            bot_id: run.bot_id.clone(),
            computer_id: run.computer_id.clone(),
            tool_name: crate::connectors::CONNECTED_APPS_EXECUTE_TOOL.to_string(),
            operation_kind,
            arguments: sanitize_connected_app_execute_arguments(&info, call_args),
            connected_app: Some(info),
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
        "run_subagent" => ToolOperationKind::Mutation,
        "recall_memory" => ToolOperationKind::Read,
        "remember" => ToolOperationKind::Mutation,
        "forget_memory" => ToolOperationKind::Mutation,
        name if is_routine_tool(name) && !is_routine_mutation_tool(name) => ToolOperationKind::Read,
        name if is_routine_mutation_tool(name) => ToolOperationKind::Mutation,
        name if is_skill_tool(name) && !is_skill_mutation_tool(name) => ToolOperationKind::Read,
        name if is_skill_mutation_tool(name) => ToolOperationKind::Mutation,
        "browser_request_human" => ToolOperationKind::Read,
        name if is_user_question_tool(name) => ToolOperationKind::Read,
        name if is_attachment_tool(name) => ToolOperationKind::Read,
        name if is_github_connector_tool(name) => ToolOperationKind::Read,
        name if is_github_coding_tool(name)
            && !is_github_coding_mutation_tool(name)
            && !crate::github_coding::is_github_coding_terminal_tool(name) =>
        {
            ToolOperationKind::Read
        }
        name if is_github_coding_mutation_tool(name) => ToolOperationKind::Mutation,
        name if is_connected_apps_tool(name) && !is_connected_apps_execute_tool(name) => {
            ToolOperationKind::Read
        }
        "connected_apps_execute_tool" => ToolOperationKind::Mutation,
        "workspace_list" | "workspace_read" => ToolOperationKind::Read,
        "browser_snapshot" => ToolOperationKind::Read,
        "workspace_write" | "workspace_exec" | "github_run_check" => ToolOperationKind::Mutation,
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
        "workspace_exec" | "github_run_check" => {
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
        "run_subagent" => json!({
            "name": args.get("name").and_then(|v| v.as_str()).unwrap_or(""),
            "taskLength": args.get("task").and_then(|v| v.as_str()).map(|s| s.len()).unwrap_or(0),
            "contextLength": args.get("context").and_then(|v| v.as_str()).map(|s| s.len()).unwrap_or(0)
        }),
        "recall_memory" => json!({
            "query": truncate_str(args.get("query").and_then(|v| v.as_str()).unwrap_or(""), 120),
            "limit": args.get("limit").and_then(|v| v.as_i64())
        }),
        "remember" => json!({
            "contentLength": args.get("content").and_then(|v| v.as_str()).map(|s| s.len()).unwrap_or(0),
            "contentPreview": args.get("content").and_then(|v| v.as_str()).map(|t| truncate_str(t, 80)),
            "kind": args.get("kind").and_then(|v| v.as_str()).unwrap_or("")
        }),
        "forget_memory" => json!({
            "memoryId": args.get("memoryId").and_then(|v| v.as_str()).unwrap_or("")
        }),
        "ask_user" => json!({
            "question": truncate_str(args.get("question").and_then(|v| v.as_str()).unwrap_or(""), 240),
            "optionCount": args.get("options").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0)
        }),
        "attachment_list" => json!({}),
        "attachment_read" => json!({
            "attachmentId": args.get("attachmentId").and_then(|v| v.as_str()).unwrap_or(""),
            "offset": args.get("offset").and_then(|v| v.as_u64()),
            "limit": args.get("limit").and_then(|v| v.as_u64())
        }),
        "connected_apps_search_tools" | "connected_apps_load_tool" => json!({
            "query": args.get("query").and_then(|v| v.as_str()).unwrap_or(""),
            "source": args.get("source").and_then(|v| v.as_str()).unwrap_or(""),
            "toolId": args.get("toolId").and_then(|v| v.as_str()).unwrap_or("")
        }),
        "routine_create" => sanitize_routine_create_arguments(args),
        "routine_pause" | "routine_resume" => sanitize_routine_toggle_arguments(args),
        "skill_save_recent_work" => sanitize_skill_save_arguments(args),
        "skill_attach" | "skill_detach" => sanitize_skill_attach_arguments(args),
        "github_open_repository" => json!({
            "owner": args.get("owner").and_then(|v| v.as_str()).unwrap_or(""),
            "repo": args.get("repo").and_then(|v| v.as_str()).unwrap_or(""),
            "taskSlug": args.get("taskSlug").and_then(|v| v.as_str()).unwrap_or("")
        }),
        "github_review_publish" => {
            let commands = args.get("checkCommands").and_then(|v| v.as_array());
            json!({
                "checkCommandCount": commands.map(|c| c.len()).unwrap_or(0)
            })
        }
        "github_publish_pull_request" => sanitize_github_publish_arguments(args),
        "connected_apps_execute_tool" => {
            let info = ConnectedAppApprovalInfo {
                install_id: args
                    .get("installId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                app_name: args
                    .get("appName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("connected app")
                    .to_string(),
                remote_tool: args
                    .get("remoteTool")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                read_only: args
                    .get("readOnly")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            };
            let call_args = args.get("arguments").unwrap_or(args);
            sanitize_connected_app_execute_arguments(&info, call_args)
        }
        _ if is_collaboration_tool(tool_name) => json!({}),
        _ => json!({}),
    }
}

fn sanitize_github_publish_arguments(args: &Value) -> Value {
    json!({
        "title": truncate_str(
            args.get("title").and_then(|v| v.as_str()).unwrap_or(""),
            120,
        ),
        "bodyLength": args.get("body").and_then(|v| v.as_str()).map(|s| s.len()).unwrap_or(0),
        "publishAnyway": args.get("publishAnyway").and_then(|v| v.as_bool()).unwrap_or(false),
        "repository": args.get("repository").and_then(|v| v.as_str()).unwrap_or(""),
        "branch": args.get("branch").and_then(|v| v.as_str()).unwrap_or(""),
        "baseBranch": args.get("baseBranch").and_then(|v| v.as_str()).unwrap_or(""),
        "changedPaths": args.get("changedPaths").cloned().unwrap_or_else(|| json!([])),
        "checksPassed": args.get("checksPassed").and_then(|v| v.as_bool()),
        "verifiedChecks": args.get("verifiedChecks").cloned().unwrap_or_else(|| json!([])),
        "explicitNoChecks": args.get("explicitNoChecks").and_then(|v| v.as_bool()).unwrap_or(false),
        "workspaceFingerprint": args.get("workspaceFingerprint").and_then(|v| v.as_str()).unwrap_or(""),
    })
}

fn sanitize_connected_app_execute_arguments(
    info: &ConnectedAppApprovalInfo,
    call_args: &Value,
) -> Value {
    json!({
        "installId": info.install_id,
        "appName": info.app_name,
        "remoteTool": info.remote_tool,
        "readOnly": info.read_only,
        "argumentSummary": summarize_connected_app_args(call_args)
    })
}

fn summarize_connected_app_args(args: &Value) -> Value {
    let Some(obj) = args.as_object() else {
        return json!({});
    };
    let mut out = serde_json::Map::new();
    for (key, value) in obj.iter().take(12) {
        let lower = key.to_ascii_lowercase();
        if lower.contains("token")
            || lower.contains("secret")
            || lower.contains("password")
            || lower.contains("authorization")
            || lower.contains("cookie")
            || lower.contains("apikey")
            || lower.contains("api_key")
        {
            out.insert(key.clone(), json!("[redacted]"));
            continue;
        }
        match value {
            Value::String(s) => {
                out.insert(key.clone(), json!(truncate_str(s, 80)));
            }
            Value::Number(n) => {
                out.insert(key.clone(), json!(n));
            }
            Value::Bool(b) => {
                out.insert(key.clone(), json!(b));
            }
            Value::Null => {
                out.insert(key.clone(), json!(null));
            }
            Value::Array(items) => {
                out.insert(key.clone(), json!({ "itemCount": items.len() }));
            }
            Value::Object(map) => {
                out.insert(key.clone(), json!({ "fieldCount": map.len() }));
            }
        }
    }
    json!(out)
}

fn sanitize_routine_create_arguments(args: &Value) -> Value {
    let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let timezone = args.get("timezone").and_then(|v| v.as_str()).unwrap_or("");
    let schedule_label = args
        .get("scheduleLabel")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            args.get("schedule")
                .and_then(routine_schedule_label_from_value)
        })
        .unwrap_or_else(|| "on a schedule".to_string());
    json!({
        "name": truncate_str(name, 80),
        "timezone": timezone,
        "scheduleLabel": schedule_label,
        "argumentSummary": {
            "name": name,
            "timezone": timezone,
            "schedule": schedule_label
        }
    })
}

fn sanitize_routine_toggle_arguments(args: &Value) -> Value {
    let routine_id = args.get("routineId").and_then(|v| v.as_str()).unwrap_or("");
    let name = args.get("routineName").and_then(|v| v.as_str()).unwrap_or("");
    let schedule_label = args
        .get("scheduleLabel")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    json!({
        "routineId": routine_id,
        "routineName": truncate_str(name, 80),
        "scheduleLabel": schedule_label,
        "argumentSummary": {
            "routineId": routine_id,
            "name": name,
            "schedule": schedule_label
        }
    })
}

fn sanitize_skill_save_arguments(args: &Value) -> Value {
    let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let description = args.get("description").and_then(|v| v.as_str()).unwrap_or("");
    let bot_name = args.get("botName").and_then(|v| v.as_str()).unwrap_or("this Bot");
    let attach = args.get("attachToBot").and_then(|v| v.as_bool()).unwrap_or(true);
    let preview = args
        .get("skillMdPreview")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    json!({
        "name": truncate_str(name, 80),
        "description": truncate_str(description, 160),
        "attachToBot": attach,
        "botName": bot_name,
        "skillMdPreview": truncate_str(preview, 480),
        "argumentSummary": {
            "name": name,
            "description": description,
            "attachToBot": attach,
            "botName": bot_name,
            "skillMdPreview": preview
        }
    })
}

fn sanitize_skill_attach_arguments(args: &Value) -> Value {
    let skill_id = args.get("skillId").and_then(|v| v.as_str()).unwrap_or("");
    let name = args.get("skillName").and_then(|v| v.as_str()).unwrap_or("");
    let bot_name = args.get("botName").and_then(|v| v.as_str()).unwrap_or("this Bot");
    json!({
        "skillId": skill_id,
        "skillName": truncate_str(name, 80),
        "botName": bot_name,
        "argumentSummary": {
            "skillId": skill_id,
            "name": name,
            "botName": bot_name
        }
    })
}

fn routine_schedule_label_from_value(schedule: &Value) -> Option<String> {
    let repeat = schedule.get("repeat").and_then(|v| v.as_str())?;
    let at = schedule.get("at").and_then(|v| v.as_str());
    match repeat {
        "every_minutes" => {
            let minutes = schedule.get("everyMinutes").and_then(|v| v.as_i64()).unwrap_or(60);
            Some(format!("every {minutes} minutes"))
        }
        "daily" => Some(format!("every day at {}", at.unwrap_or("08:00"))),
        "weekdays" => Some(format!("every weekday at {}", at.unwrap_or("08:00"))),
        "weekly" => {
            let days = schedule
                .get("days")
                .and_then(|v| v.as_str())
                .unwrap_or("weekdays");
            Some(format!("{days} at {}", at.unwrap_or("08:00")))
        }
        _ => None,
    }
}

pub fn approval_action_summary(tool_name: &str, sanitized: &Value) -> String {
    match tool_name {
        "routine_create" => {
            let name = sanitized.get("name").and_then(|v| v.as_str()).unwrap_or("routine");
            let schedule = sanitized
                .get("scheduleLabel")
                .and_then(|v| v.as_str())
                .unwrap_or("on a schedule");
            let timezone = sanitized.get("timezone").and_then(|v| v.as_str()).unwrap_or("UTC");
            format!("Run \"{name}\" {schedule} ({timezone})")
        }
        "routine_pause" => {
            let name = sanitized
                .get("routineName")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("this routine");
            format!("Pause \"{name}\"")
        }
        "routine_resume" => {
            let name = sanitized
                .get("routineName")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("this routine");
            format!("Resume \"{name}\"")
        }
        "skill_save_recent_work" => {
            let name = sanitized.get("name").and_then(|v| v.as_str()).unwrap_or("skill");
            let bot = sanitized.get("botName").and_then(|v| v.as_str()).unwrap_or("this Bot");
            let attach = sanitized.get("attachToBot").and_then(|v| v.as_bool()).unwrap_or(true);
            if attach {
                format!("Save \"{name}\" and attach to {bot}")
            } else {
                format!("Save \"{name}\"")
            }
        }
        "skill_attach" => {
            let name = sanitized
                .get("skillName")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("this skill");
            let bot = sanitized.get("botName").and_then(|v| v.as_str()).unwrap_or("this Bot");
            format!("Attach \"{name}\" to {bot}")
        }
        "skill_detach" => {
            let name = sanitized
                .get("skillName")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("this skill");
            let bot = sanitized.get("botName").and_then(|v| v.as_str()).unwrap_or("this Bot");
            format!("Remove \"{name}\" from {bot}")
        }
        "workspace_write" => {
            let path = sanitized
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("/workspace");
            format!("Write {path}")
        }
        "workspace_exec" | "github_run_check" => {
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
        "run_subagent" => {
            let name = sanitized
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("helper");
            format!("Run subagent {name}")
        }
        "github_publish_pull_request" => {
            let repo = sanitized
                .get("repository")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("repository");
            let branch = sanitized
                .get("branch")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("branch");
            format!("Publish {repo} ({branch}) to GitHub")
        }
        "connected_apps_execute_tool" => {
            let app = sanitized
                .get("appName")
                .and_then(|v| v.as_str())
                .unwrap_or("a connected app");
            let tool = sanitized
                .get("remoteTool")
                .and_then(|v| v.as_str())
                .unwrap_or("tool");
            format!("Use {app}: {tool}")
        }
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
            operation_kind_for_tool("connected_apps_search_tools"),
            ToolOperationKind::Read
        );
        assert_eq!(
            operation_kind_for_tool("connected_apps_load_tool"),
            ToolOperationKind::Read
        );
        assert_eq!(
            operation_kind_for_tool("connected_apps_execute_tool"),
            ToolOperationKind::Mutation
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
            operation_kind_for_tool("bot_delegate"),
            ToolOperationKind::Mutation
        );
        assert_eq!(
            operation_kind_for_tool("run_subagent"),
            ToolOperationKind::Mutation
        );
        assert_eq!(
            operation_kind_for_tool("recall_memory"),
            ToolOperationKind::Read
        );
        assert_eq!(
            operation_kind_for_tool("remember"),
            ToolOperationKind::Mutation
        );
        assert_eq!(
            operation_kind_for_tool("forget_memory"),
            ToolOperationKind::Mutation
        );
        assert_eq!(
            operation_kind_for_tool("github_run_check"),
            ToolOperationKind::Mutation
        );
        assert_eq!(
            operation_kind_for_tool("github_open_repository"),
            ToolOperationKind::Read
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
                connected_app: None,
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

        let sanitized = sanitize_tool_arguments("browser_type", &json!({"ref":"e1","text": text}));
        assert_eq!(sanitized.get("textPreview"), Some(&json!("a".repeat(79))));
    }

    #[test]
    fn sanitize_write_strips_content() {
        let sanitized = sanitize_tool_arguments(
            "workspace_write",
            &json!({"path":"/workspace/a.txt","content":"hello"}),
        );
        assert_eq!(sanitized.get("contentLength"), Some(&json!(5)));
        assert_eq!(sanitized.get("contentPreview"), Some(&json!("hello")));
    }
}
