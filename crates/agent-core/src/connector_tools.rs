use serde_json::{json, Value};

use crate::approval::{ToolApprovalContext, ToolApprovalGate, ToolRunContext};
use crate::connectors::{
    bound_connector_tool_result, truncate_connector_tool_result, AgentConnectors, ConnectorError,
    CONNECTED_APPS_EXECUTE_TOOL, CONNECTED_APPS_LOAD_TOOL, CONNECTED_APPS_SEARCH_TOOL,
    MAX_CONNECTED_APP_SEARCH_LIMIT,
};
use crate::tool_catalog::is_github_connector_tool;
use crate::tools::ToolError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub fn connector_openai_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "name": "github_list_repositories",
            "description": "List repositories authorized through the connected GitHub App installations. Read-only.",
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
            "description": "Search repositories authorized through the connected GitHub App installations. Read-only.",
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
        json!({
            "type": "function",
            "name": crate::connectors::CONNECTED_APPS_SEARCH_TOOL,
            "description": "Search installed connected-app tools the owner authorized for this Bot. Use this before load/execute. Empty query returns a bounded index.",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Optional search text" },
                    "source": { "type": "string", "description": "Optional connected app display name filter" },
                    "limit": { "type": "integer", "description": "Maximum results (default 20)" }
                },
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": crate::connectors::CONNECTED_APPS_LOAD_TOOL,
            "description": "Load the exact JSON schema for one connected-app tool using the opaque id from search.",
            "parameters": {
                "type": "object",
                "properties": {
                    "toolId": { "type": "string", "description": "Opaque tool id from connected_apps_search_tools" }
                },
                "required": ["toolId"],
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": crate::connectors::CONNECTED_APPS_EXECUTE_TOOL,
            "description": "Execute one connected-app tool previously returned by search/load. The id is resolved server-side; do not invent endpoints or installation ids.",
            "parameters": {
                "type": "object",
                "properties": {
                    "toolId": { "type": "string", "description": "Opaque tool id from connected_apps_search_tools or connected_apps_load_tool" },
                    "arguments": { "type": "object", "description": "Arguments matching the loaded schema" }
                },
                "required": ["toolId", "arguments"],
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

    let args: Value = serde_json::from_str(arguments)
        .map_err(|e| ToolError::MalformedArguments(format!("invalid JSON arguments: {e}")))?;

    if is_github_connector_tool(name)
        || name == CONNECTED_APPS_SEARCH_TOOL
        || name == CONNECTED_APPS_LOAD_TOOL
    {
        let approval_ctx = ToolApprovalContext::for_tool(run, name, args.clone());
        authorize(gate, &approval_ctx).await?;
        if cancel.load(Ordering::Relaxed) {
            return Err(ToolError::Cancelled);
        }
        let result = match name {
            CONNECTED_APPS_SEARCH_TOOL => {
                let query = args.get("query").and_then(|v| v.as_str());
                let source = args.get("source").and_then(|v| v.as_str());
                let limit = args
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|n| n.min(MAX_CONNECTED_APP_SEARCH_LIMIT as u64) as u32);
                service
                    .search_connected_app_tools(&run.owner_id, query, source, limit)
                    .await
                    .map_err(map_connector_error)?
            }
            CONNECTED_APPS_LOAD_TOOL => {
                let tool_id = required_tool_id(&args)?;
                let loaded = service
                    .load_connected_app_tool(&run.owner_id, tool_id)
                    .await
                    .map_err(map_connector_error)?;
                json!({
                    "ok": true,
                    "toolId": loaded.id,
                    "name": loaded.name,
                    "source": loaded.source,
                    "description": loaded.description,
                    "inputSchema": loaded.input_schema,
                    "readOnly": loaded.read_only
                })
            }
            _ => service
                .dispatch_connector_tool(&run.owner_id, name, &args)
                .await
                .map_err(map_connector_error)?,
        };
        let result = bound_connector_tool_result(result).map_err(map_connector_error)?;
        return Ok(with_tool_name(result, name));
    }

    if name == CONNECTED_APPS_EXECUTE_TOOL {
        let tool_id = required_tool_id(&args)?;
        if args.get("endpoint").is_some()
            || args.get("installId").is_some()
            || args.get("url").is_some()
            || args.get("remoteTool").is_some()
        {
            return Err(ToolError::MalformedArguments(
                "connected app execute accepts only toolId and arguments".into(),
            ));
        }
        let call_args = args.get("arguments").cloned().unwrap_or_else(|| json!({}));
        if !call_args.is_object() {
            return Err(ToolError::MalformedArguments(
                "`arguments` must be an object".into(),
            ));
        }
        let loaded = service
            .load_connected_app_tool(&run.owner_id, tool_id)
            .await
            .map_err(map_connector_error)?;
        let approval_ctx = ToolApprovalContext::for_connected_app_execute(run, &loaded, &call_args);
        authorize(gate, &approval_ctx).await?;
        if cancel.load(Ordering::Relaxed) {
            return Err(ToolError::Cancelled);
        }
        let result = service
            .execute_connected_app_tool(&run.owner_id, tool_id, &call_args)
            .await
            .map_err(map_connector_error)?;
        let result = truncate_connector_tool_result(result).map_err(map_connector_error)?;
        return Ok(with_tool_name(result, name));
    }

    Err(ToolError::MalformedArguments(format!(
        "unknown connector tool: {name}"
    )))
}

async fn authorize(
    gate: &dyn ToolApprovalGate,
    approval_ctx: &ToolApprovalContext,
) -> Result<(), ToolError> {
    let approval = gate
        .authorize(approval_ctx)
        .await
        .map_err(map_approval_error)?;
    if let crate::approval::ApprovalDecision::Deny { reason } = approval {
        return Err(ToolError::Denied(reason));
    }
    Ok(())
}

fn required_tool_id(args: &Value) -> Result<&str, ToolError> {
    args.get("toolId")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ToolError::MalformedArguments("missing or empty `toolId`".into()))
}

fn with_tool_name(mut result: Value, name: &str) -> Value {
    if let Some(obj) = result.as_object_mut() {
        obj.insert("tool".into(), json!(name));
    }
    result
}

fn map_connector_error(err: ConnectorError) -> ToolError {
    match err {
        ConnectorError::NotConnected => ToolError::Denied(err.message()),
        ConnectorError::ReconnectRequired => ToolError::Denied(err.message()),
        ConnectorError::NotFound => ToolError::MalformedArguments(err.message()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approval::AllowAllApprovalGate;
    use crate::connectors::ConnectorToolDefinition;
    use async_trait::async_trait;
    use std::sync::atomic::AtomicBool;

    struct FakeConnectors {
        owner: String,
        tool: ConnectorToolDefinition,
    }

    #[async_trait]
    impl AgentConnectors for FakeConnectors {
        async fn dispatch_connector_tool(
            &self,
            _owner_id: &str,
            _tool_name: &str,
            _arguments: &Value,
        ) -> Result<Value, ConnectorError> {
            Err(ConnectorError::Validation("github unused".into()))
        }

        async fn search_connected_app_tools(
            &self,
            owner_id: &str,
            _query: Option<&str>,
            _source: Option<&str>,
            _limit: Option<u32>,
        ) -> Result<Value, ConnectorError> {
            if owner_id != self.owner {
                return Ok(json!({ "ok": true, "tools": [] }));
            }
            Ok(json!({
                "ok": true,
                "tools": [{
                    "toolId": self.tool.id,
                    "name": self.tool.name,
                    "source": self.tool.source,
                    "description": self.tool.description,
                    "readOnly": self.tool.read_only
                }]
            }))
        }

        async fn load_connected_app_tool(
            &self,
            owner_id: &str,
            tool_id: &str,
        ) -> Result<ConnectorToolDefinition, ConnectorError> {
            if owner_id != self.owner || tool_id != self.tool.id {
                return Err(ConnectorError::NotFound);
            }
            Ok(self.tool.clone())
        }

        async fn execute_connected_app_tool(
            &self,
            owner_id: &str,
            tool_id: &str,
            arguments: &Value,
        ) -> Result<Value, ConnectorError> {
            if owner_id != self.owner || tool_id != self.tool.id {
                return Err(ConnectorError::NotFound);
            }
            Ok(json!({ "ok": true, "echo": arguments }))
        }
    }

    fn sample_tool() -> ConnectorToolDefinition {
        ConnectorToolDefinition {
            id: "11111111-1111-1111-1111-111111111111".into(),
            install_id: "22222222-2222-2222-2222-222222222222".into(),
            name: "search_docs".into(),
            source: "Docs".into(),
            description: "Search".into(),
            input_schema: json!({ "type": "object" }),
            read_only: true,
            kind: "mcp".into(),
        }
    }

    fn run() -> ToolRunContext {
        ToolRunContext {
            run_id: "run".into(),
            request_id: "req".into(),
            owner_id: "alice".into(),
            bot_id: "bot".into(),
            computer_id: "comp".into(),
            tool_invocation_id: None,
        }
    }

    #[tokio::test]
    async fn catalog_search_load_execute_and_rejects_overrides() {
        let connectors: Arc<dyn AgentConnectors> = Arc::new(FakeConnectors {
            owner: "alice".into(),
            tool: sample_tool(),
        });
        let gate = AllowAllApprovalGate;
        let cancel = AtomicBool::new(false);
        let search = dispatch_connector_tool_with_gate(
            Some(&connectors),
            CONNECTED_APPS_SEARCH_TOOL,
            "{}",
            &cancel,
            &gate,
            &run(),
        )
        .await
        .unwrap();
        assert_eq!(
            search
                .get("tools")
                .and_then(|v| v.as_array())
                .unwrap()
                .len(),
            1
        );

        let loaded = dispatch_connector_tool_with_gate(
            Some(&connectors),
            CONNECTED_APPS_LOAD_TOOL,
            r#"{"toolId":"11111111-1111-1111-1111-111111111111"}"#,
            &cancel,
            &gate,
            &run(),
        )
        .await
        .unwrap();
        assert_eq!(loaded.get("name"), Some(&json!("search_docs")));

        let executed = dispatch_connector_tool_with_gate(
            Some(&connectors),
            CONNECTED_APPS_EXECUTE_TOOL,
            r#"{"toolId":"11111111-1111-1111-1111-111111111111","arguments":{"q":"hi"}}"#,
            &cancel,
            &gate,
            &run(),
        )
        .await
        .unwrap();
        assert_eq!(executed.get("ok"), Some(&json!(true)));

        let forged = dispatch_connector_tool_with_gate(
            Some(&connectors),
            CONNECTED_APPS_EXECUTE_TOOL,
            r#"{"toolId":"99999999-9999-9999-9999-999999999999","arguments":{}}"#,
            &cancel,
            &gate,
            &run(),
        )
        .await
        .unwrap_err();
        assert!(matches!(forged, ToolError::MalformedArguments(_)));

        let override_err = dispatch_connector_tool_with_gate(
            Some(&connectors),
            CONNECTED_APPS_EXECUTE_TOOL,
            r#"{"toolId":"11111111-1111-1111-1111-111111111111","arguments":{},"endpoint":"https://evil"}"#,
            &cancel,
            &gate,
            &run(),
        )
        .await
        .unwrap_err();
        assert!(matches!(override_err, ToolError::MalformedArguments(_)));
    }

    #[test]
    fn catalog_definitions_are_exactly_three_plus_github() {
        let names: Vec<_> = connector_openai_tool_definitions()
            .into_iter()
            .map(|v| v.get("name").and_then(|n| n.as_str()).unwrap().to_string())
            .collect();
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
        assert_eq!(
            names
                .iter()
                .filter(|n| n.starts_with("connected_apps_"))
                .count(),
            3
        );
        assert!(!names.iter().any(|n| n == "search_docs"));
    }
}
