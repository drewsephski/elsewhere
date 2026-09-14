use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use agent_core::{dispatch_tool_with_gate, AgentComputer, ToolApprovalGate, ToolRunContext, ToolError};
use rmcp::{
    ErrorData, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ListToolsResult,
        ServerCapabilities, ServerInfo, Tool,
    },
    service::RequestContext,
    RoleServer,
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
}

impl ComputerHandler {
    pub fn new(
        computer: Arc<dyn AgentComputer>,
        gate: Arc<dyn ToolApprovalGate>,
        run: ToolRunContext,
        cancel: Arc<AtomicBool>,
    ) -> Self {
        Self {
            computer,
            cancel,
            gate,
            run,
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

fn tool_definitions() -> Vec<Tool> {
    vec![
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
    ]
}

impl ServerHandler for ComputerHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(
                "Elsewhere computer tools. All paths must be under /workspace on the agent VM.",
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
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let args = request
            .arguments
            .map(serde_json::Value::Object)
            .unwrap_or_else(|| json!({}));
        if let Err(err) = validate_tool_args(&request.name, &args) {
            return Ok(tool_error_result(err));
        }

        let args_str = args.to_string();
        let dispatch = dispatch_tool_with_gate(
            self.computer.as_ref(),
            &request.name,
            &args_str,
            self.cancel.as_ref(),
            self.gate.as_ref(),
            &self.run,
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
    }
}
