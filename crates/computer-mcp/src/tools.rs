use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use agent_core::{
    connector_openai_tool_definitions, dispatch_agent_tool_with_gate_and_recovery,
    human_intervention_openai_tool_definitions, is_attachment_tool, is_browser_tool,
    is_collaboration_tool, is_connector_tool, is_human_intervention_tool, is_memory_tool,
    github_coding_openai_tool_definitions, is_routine_tool, is_skill_tool, is_subagent_tool,
    is_user_question_tool, AgentAttachments, AgentCollaboration, AgentGithubCoding,
    AgentComputer, AgentConnectors, AgentHumanIntervention, AgentMemory, AgentRoutines,
    AgentSkills, AgentSubagents, AgentUserQuestion,
    BrowserRecoverySession, CollaborationContext, ToolApprovalGate, ToolError, ToolRunContext,
    RUN_SUBAGENT_DESCRIPTION,
};
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ListToolsResult,
        ServerCapabilities, ServerInfo, Tool,
    },
    service::RequestContext,
    ErrorData, RoleServer, ServerHandler,
};
use serde_json::json;

use crate::error::ComputerMcpError;
use crate::path_policy::require_workspace_path;

const MAX_TOOL_JSON_BYTES: usize = 512 * 1024;

#[derive(Clone)]
pub struct ComputerHandler {
    pub computer: Arc<dyn AgentComputer>,
    cancel: Arc<AtomicBool>,
    gate: Arc<dyn ToolApprovalGate>,
    run: ToolRunContext,
    collaboration: Option<Arc<dyn AgentCollaboration>>,
    connectors: Option<Arc<dyn AgentConnectors>>,
    human_intervention: Option<Arc<dyn AgentHumanIntervention>>,
    browser_recovery: Option<Arc<BrowserRecoverySession>>,
    subagents: Option<Arc<dyn AgentSubagents>>,
    memory: Option<Arc<dyn AgentMemory>>,
    routines: Option<Arc<dyn AgentRoutines>>,
    skills: Option<Arc<dyn AgentSkills>>,
    github_coding: Option<Arc<dyn AgentGithubCoding>>,
    attachments: Option<Arc<dyn AgentAttachments>>,
    user_questions: Option<Arc<dyn AgentUserQuestion>>,
    source_conversation_id: String,
}

impl ComputerHandler {
    pub fn new(
        computer: Arc<dyn AgentComputer>,
        gate: Arc<dyn ToolApprovalGate>,
        run: ToolRunContext,
        cancel: Arc<AtomicBool>,
        collaboration: Option<Arc<dyn AgentCollaboration>>,
        connectors: Option<Arc<dyn AgentConnectors>>,
        human_intervention: Option<Arc<dyn AgentHumanIntervention>>,
        browser_recovery: Option<Arc<BrowserRecoverySession>>,
        subagents: Option<Arc<dyn AgentSubagents>>,
        memory: Option<Arc<dyn AgentMemory>>,
        routines: Option<Arc<dyn AgentRoutines>>,
        skills: Option<Arc<dyn AgentSkills>>,
        github_coding: Option<Arc<dyn AgentGithubCoding>>,
        attachments: Option<Arc<dyn AgentAttachments>>,
        user_questions: Option<Arc<dyn AgentUserQuestion>>,
        source_conversation_id: String,
    ) -> Self {
        Self {
            computer,
            cancel,
            gate,
            run,
            collaboration,
            connectors,
            human_intervention,
            browser_recovery,
            subagents,
            memory,
            routines,
            skills,
            github_coding,
            attachments,
            user_questions,
            source_conversation_id,
        }
    }
}

fn schema_object(value: serde_json::Value) -> Arc<serde_json::Map<String, serde_json::Value>> {
    Arc::new(
        value
            .as_object()
            .cloned()
            .expect("tool schema must be object"),
    )
}

fn collaboration_tool_definitions() -> Vec<Tool> {
    vec![
        Tool::new(
            "bot_list",
            "List other Bots owned by the same user that you may hand work to asynchronously. Does not wait for them to finish.",
            schema_object(json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            })),
        ),
        Tool::new(
            "bot_delegate",
            "Hand work to another Bot asynchronously. This only queues durable work; it does NOT wait for completion.",
            schema_object(json!({
                "type": "object",
                "properties": {
                    "targetBotId": { "type": "string" },
                    "instruction": { "type": "string" },
                    "context": { "type": "string" }
                },
                "required": ["targetBotId", "instruction"],
                "additionalProperties": false
            })),
        ),
        Tool::new(
            "run_subagent",
            RUN_SUBAGENT_DESCRIPTION,
            schema_object(json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "task": { "type": "string" },
                    "context": { "type": "string" }
                },
                "required": ["name", "task"],
                "additionalProperties": false
            })),
        ),
    ]
}

fn tool_definitions() -> Vec<Tool> {
    let mut tools = vec![
        Tool::new(
            "workspace_list",
            "List files and directories under a workspace path inside the agent computer.",
            schema_object(json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"],
                "additionalProperties": false
            })),
        ),
        Tool::new(
            "workspace_read",
            "Read a text file from the workspace.",
            schema_object(json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"],
                "additionalProperties": false
            })),
        ),
        Tool::new(
            "workspace_write",
            "Write or overwrite a text file in the workspace.",
            schema_object(json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"],
                "additionalProperties": false
            })),
        ),
        Tool::new(
            "workspace_exec",
            "Run a shell command inside the sandboxed Linux workspace (not on the host).",
            schema_object(json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string" }
                },
                "required": ["command"],
                "additionalProperties": false
            })),
        ),
        Tool::new(
            "browser_navigate",
            "Open a URL in the agent computer headless browser.",
            schema_object(json!({
                "type": "object",
                "properties": { "url": { "type": "string" } },
                "required": ["url"],
                "additionalProperties": false
            })),
        ),
        Tool::new(
            "browser_snapshot",
            "List interactive elements on the current page with refs for click/type.",
            schema_object(json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            })),
        ),
        Tool::new(
            "browser_click",
            "Click an element by ref from browser_snapshot.",
            schema_object(json!({
                "type": "object",
                "properties": { "ref": { "type": "string" } },
                "required": ["ref"],
                "additionalProperties": false
            })),
        ),
        Tool::new(
            "browser_type",
            "Type into an element by ref from browser_snapshot.",
            schema_object(json!({
                "type": "object",
                "properties": {
                    "ref": { "type": "string" },
                    "text": { "type": "string" },
                    "submit": { "type": "boolean" }
                },
                "required": ["ref", "text"],
                "additionalProperties": false
            })),
        ),
        Tool::new(
            "browser_screenshot",
            "Save a PNG screenshot under /workspace.",
            schema_object(json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "fullPage": { "type": "boolean" }
                },
                "required": ["path"],
                "additionalProperties": false
            })),
        ),
        Tool::new(
            "browser_download",
            "Download a URL into a workspace file.",
            schema_object(json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string" },
                    "path": { "type": "string" }
                },
                "required": ["url", "path"],
                "additionalProperties": false
            })),
        ),
    ];
    tools.extend(collaboration_tool_definitions());
    tools.extend(
        human_intervention_openai_tool_definitions()
            .into_iter()
            .map(|value| {
                let name = value
                    .get("name")
                    .and_then(|v| v.as_str())
                    .expect("human intervention tool name")
                    .to_string();
                let description = value
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Request owner browser help")
                    .to_string();
                let parameters = value
                    .get("parameters")
                    .cloned()
                    .unwrap_or_else(|| json!({"type": "object"}));
                Tool::new(name, description, schema_object(parameters))
            })
            .collect::<Vec<_>>(),
    );
    tools.extend(connector_mcp_tool_definitions());
    tools.extend(memory_mcp_tool_definitions());
    tools.extend(routine_mcp_tool_definitions());
    tools.extend(openai_mcp_tools(
        agent_core::skill_openai_tool_definitions(),
        "Manage Agent Skills",
    ));
    tools.extend(openai_mcp_tools(
        github_coding_openai_tool_definitions(),
        "GitHub repository coding workflow",
    ));
    tools.extend(openai_mcp_tools(
        agent_core::user_question_openai_tool_definitions(),
        "Ask the owner a multiple-choice question",
    ));
    tools.extend(openai_mcp_tools(
        agent_core::attachment_openai_tool_definitions(),
        "Read owner attachments",
    ));
    tools
}

fn openai_mcp_tools(defs: Vec<serde_json::Value>, fallback: &str) -> Vec<Tool> {
    defs.into_iter()
        .map(|value| {
            let name = value
                .get("name")
                .and_then(|v| v.as_str())
                .expect("tool name")
                .to_string();
            let description = value
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or(fallback)
                .to_string();
            let parameters = value
                .get("parameters")
                .cloned()
                .unwrap_or_else(|| json!({"type": "object"}));
            Tool::new(name, description, schema_object(parameters))
        })
        .collect()
}

fn routine_mcp_tool_definitions() -> Vec<Tool> {
    agent_core::routine_openai_tool_definitions()
        .into_iter()
        .map(|value| {
            let name = value
                .get("name")
                .and_then(|v| v.as_str())
                .expect("routine tool name")
                .to_string();
            let description = value
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("Routine tool")
                .to_string();
            let parameters = value
                .get("parameters")
                .cloned()
                .unwrap_or_else(|| json!({"type": "object"}));
            Tool::new(name, description, schema_object(parameters))
        })
        .collect()
}

fn memory_mcp_tool_definitions() -> Vec<Tool> {
    agent_core::memory_openai_tool_definitions()
        .into_iter()
        .map(|value| {
            let name = value
                .get("name")
                .and_then(|v| v.as_str())
                .expect("memory tool name")
                .to_string();
            let description = value
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("Memory tool")
                .to_string();
            let parameters = value
                .get("parameters")
                .cloned()
                .unwrap_or_else(|| json!({"type": "object"}));
            Tool::new(name, description, schema_object(parameters))
        })
        .collect()
}

fn connector_mcp_tool_definitions() -> Vec<Tool> {
    connector_openai_tool_definitions()
        .into_iter()
        .map(|value| {
            let name = value
                .get("name")
                .and_then(|v| v.as_str())
                .expect("connector tool name")
                .to_string();
            let description = value
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("Connector tool")
                .to_string();
            let parameters = value
                .get("parameters")
                .cloned()
                .unwrap_or_else(|| json!({"type": "object"}));
            Tool::new(name, description, schema_object(parameters))
        })
        .collect()
}

impl ServerHandler for ComputerHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(
                "Elsewhere computer tools. Paths must be under /workspace. Browser tools run headless Chromium inside the agent VM, not on the runner host.",
            )
    }

    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        Ok(ListToolsResult {
            tools: tool_definitions(),
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let args = request
            .arguments
            .map(serde_json::Value::Object)
            .unwrap_or_else(|| json!({}));
        if let Err(err) = validate_tool_args(&request.name, &args) {
            return Ok(tool_error_result(err));
        }

        let args_str = args.to_string();
        let invocation_id = format!("mcp:{}", context.id);
        let tool_run = ToolRunContext {
            run_id: self.run.run_id.clone(),
            request_id: self.run.request_id.clone(),
            owner_id: self.run.owner_id.clone(),
            bot_id: self.run.bot_id.clone(),
            computer_id: self.run.computer_id.clone(),
            tool_invocation_id: Some(invocation_id.clone()),
        };
        let collaboration_ctx = CollaborationContext {
            owner_id: self.run.owner_id.clone(),
            source_bot_id: self.run.bot_id.clone(),
            source_run_id: self.run.run_id.clone(),
            source_conversation_id: self.source_conversation_id.clone(),
            source_request_id: self.run.request_id.clone(),
            tool_invocation_id: invocation_id,
        };
        let dispatch = dispatch_agent_tool_with_gate_and_recovery(
            self.computer.as_ref(),
            self.collaboration.as_ref(),
            self.connectors.as_ref(),
            self.human_intervention.as_ref(),
            self.subagents.as_ref(),
            self.memory.as_ref(),
            self.routines.as_ref(),
            self.skills.as_ref(),
            self.github_coding.as_ref(),
            self.attachments.as_ref(),
            self.user_questions.as_ref(),
            &request.name,
            &args_str,
            self.cancel.as_ref(),
            self.gate.as_ref(),
            &tool_run,
            Some(&collaboration_ctx),
            self.browser_recovery.as_ref(),
        )
        .await;

        match dispatch {
            Ok(value) => {
                let text = serde_json::to_string(&value).unwrap_or_else(|_| "{}".into());
                if text.len() > MAX_TOOL_JSON_BYTES {
                    return Ok(tool_error_result(ComputerMcpError::ExecutionFailed(
                        "tool output too large".into(),
                    )));
                }
                Ok(CallToolResult::success(vec![ContentBlock::text(text)]).into())
            }
            Err(tool_err) => Ok(tool_error_result(map_tool_error(tool_err))),
        }
    }
}

fn validate_tool_args(name: &str, args: &serde_json::Value) -> Result<(), ComputerMcpError> {
    match name {
        "workspace_list" | "workspace_read" | "workspace_write" => {
            let path = args
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ComputerMcpError::MalformedArguments("missing path".into()))?;
            require_workspace_path(path)?;
            Ok(())
        }
        "workspace_exec" => Ok(()),
        "browser_screenshot" | "browser_download" => {
            let path = args
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ComputerMcpError::MalformedArguments("missing path".into()))?;
            require_workspace_path(path)?;
            Ok(())
        }
        name if is_browser_tool(name) => Ok(()),
        "bot_list" => Ok(()),
        "bot_delegate" => Ok(()),
        "run_subagent" => Ok(()),
        name if is_human_intervention_tool(name) => Ok(()),
        name if is_collaboration_tool(name) => Ok(()),
        name if is_subagent_tool(name) => Ok(()),
        name if is_connector_tool(name) => Ok(()),
        name if is_memory_tool(name) => Ok(()),
        name if is_routine_tool(name) => Ok(()),
        name if is_skill_tool(name) => Ok(()),
        name if is_attachment_tool(name) => Ok(()),
        name if is_user_question_tool(name) => Ok(()),
        other => Err(ComputerMcpError::MalformedArguments(format!(
            "unknown tool: {other}"
        ))),
    }
}

fn map_tool_error(err: ToolError) -> ComputerMcpError {
    match err {
        ToolError::ComputerNotReady(e) => ComputerMcpError::from_computer(e),
        ToolError::MalformedArguments(d) => ComputerMcpError::MalformedArguments(d),
        ToolError::Denied(d) => ComputerMcpError::MalformedArguments(d),
        ToolError::Cancelled => ComputerMcpError::Cancelled,
    }
}

fn tool_error_result(err: ComputerMcpError) -> CallToolResponse {
    CallToolResult::error(vec![ContentBlock::text(err.to_string())]).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_schemas_include_required_names() {
        let names: Vec<_> = tool_definitions()
            .into_iter()
            .map(|t| t.name.to_string())
            .collect();
        assert!(names.contains(&"workspace_list".to_string()));
        assert!(names.contains(&"workspace_exec".to_string()));
        assert!(names.contains(&"run_subagent".to_string()));
        assert!(names.contains(&"bot_delegate".to_string()));
        assert!(names.contains(&"connected_apps_search_tools".to_string()));
        assert!(names.contains(&"connected_apps_load_tool".to_string()));
        assert!(names.contains(&"connected_apps_execute_tool".to_string()));
        assert!(names.contains(&"github_list_repositories".to_string()));
        for name in [
            "github_search_repositories",
            "github_get_repository",
            "github_get_file_contents",
            "github_list_issues",
            "github_get_issue",
            "github_list_pull_requests",
            "github_get_pull_request",
        ] {
            assert!(names.contains(&name.to_string()), "{name}");
        }
        assert!(names.contains(&"routine_list".to_string()));
        assert!(names.contains(&"routine_create".to_string()));
        assert!(names.contains(&"recall_memory".to_string()));
        assert!(names.contains(&"remember".to_string()));
        assert!(names.contains(&"forget_memory".to_string()));
        assert!(names.contains(&"ask_user".to_string()));
        assert!(names.contains(&"attachment_list".to_string()));
        assert!(names.contains(&"attachment_read".to_string()));
        assert_eq!(
            names
                .iter()
                .filter(|n| n.starts_with("connected_apps_"))
                .count(),
            3
        );
    }
}
