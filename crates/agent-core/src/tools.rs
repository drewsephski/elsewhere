use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::approval::{
    AllowAllApprovalGate, ApprovalDecision, ApprovalError, ToolApprovalContext, ToolApprovalGate,
    ToolRunContext, MAX_EXEC_COMMAND_CHARS,
};
use crate::browser_tools::{browser_openai_tool_definitions, dispatch_browser_tool};
use crate::computer::{AgentComputer, ComputerError};
use crate::tool_catalog::is_browser_tool;

pub const MAX_AGENT_TOOL_STEPS: usize = 25;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolError {
    ComputerNotReady(ComputerError),
    MalformedArguments(String),
    Denied(String),
    Cancelled,
}

impl ToolError {
    pub fn code(&self) -> &'static str {
        match self {
            ToolError::ComputerNotReady(err) => err.code(),
            ToolError::MalformedArguments(_) => "malformed_tool_arguments",
            ToolError::Denied(_) => "tool_denied",
            ToolError::Cancelled => "cancelled",
        }
    }

    pub fn message(&self) -> String {
        match self {
            ToolError::ComputerNotReady(err) => err.to_string(),
            ToolError::MalformedArguments(d) => d.clone(),
            ToolError::Denied(d) => d.clone(),
            ToolError::Cancelled => "Agent run cancelled".into(),
        }
    }

    pub fn computer_error(&self) -> Option<&ComputerError> {
        match self {
            ToolError::ComputerNotReady(e) => Some(e),
            _ => None,
        }
    }
}

pub fn openai_tool_definitions() -> Vec<Value> {
    let mut tools = vec![
        json!({
            "type": "function",
            "name": "workspace_list",
            "description": "List files and directories under a workspace path inside the Bot's Linux computer.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory path, typically /workspace" }
                },
                "required": ["path"],
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": "workspace_read",
            "description": "Read a text file from the workspace.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"],
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": "workspace_write",
            "description": "Write or overwrite a text file in the workspace.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"],
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": "workspace_exec",
            "description": "Run a shell command inside the sandboxed Linux workspace (not on the Mac host).",
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
    ];
    tools.extend(browser_openai_tool_definitions());
    tools
}

pub async fn dispatch_tool(
    computer: &dyn AgentComputer,
    name: &str,
    arguments: &str,
    cancel: &AtomicBool,
    run: &ToolRunContext,
) -> Result<Value, ToolError> {
    dispatch_tool_with_gate(
        computer,
        name,
        arguments,
        cancel,
        &AllowAllApprovalGate,
        run,
    )
    .await
}

pub async fn dispatch_tool_with_gate(
    computer: &dyn AgentComputer,
    name: &str,
    arguments: &str,
    cancel: &AtomicBool,
    gate: &dyn ToolApprovalGate,
    run: &ToolRunContext,
) -> Result<Value, ToolError> {
    if cancel.load(Ordering::Relaxed) {
        return Err(ToolError::Cancelled);
    }

    let args: Value = serde_json::from_str(arguments).map_err(|e| {
        ToolError::MalformedArguments(format!("invalid JSON arguments: {e}"))
    })?;

    validate_tool_argument_limits(name, &args)?;

    let approval_ctx = ToolApprovalContext::for_tool(run, name, args.clone());
    let approval = gate.authorize(&approval_ctx).await.map_err(map_approval_error)?;
    if let ApprovalDecision::Deny { reason } = approval {
        return Err(ToolError::Denied(reason));
    }

    computer
        .ensure_ready()
        .await
        .map_err(ToolError::ComputerNotReady)?;

    if cancel.load(Ordering::Relaxed) {
        return Err(ToolError::Cancelled);
    }

    let started = Instant::now();
    let result = if is_browser_tool(name) {
        dispatch_browser_tool(computer, name, &args).await
    } else {
        match name {
            "workspace_list" => workspace_list(computer, &args).await,
            "workspace_read" => workspace_read(computer, &args).await,
            "workspace_write" => workspace_write(computer, &args).await,
            "workspace_exec" => workspace_exec(computer, &args).await,
            other => Err(ToolError::MalformedArguments(format!("unknown tool: {other}"))),
        }
    };

    if let Err(ToolError::ComputerNotReady(_)) = &result {
        computer.invalidate_cached_readiness();
    }

    let result = result?;

    let mut envelope = result;
    if let Some(obj) = envelope.as_object_mut() {
        obj.insert("durationMs".into(), json!(started.elapsed().as_millis()));
    }
    Ok(envelope)
}

async fn workspace_list(computer: &dyn AgentComputer, args: &Value) -> Result<Value, ToolError> {
    let path = required_str(args, "path")?;
    let entries = computer
        .list_dir(path)
        .await
        .map_err(ToolError::ComputerNotReady)?;
    Ok(json!({
        "ok": true,
        "path": path,
        "entries": entries
    }))
}

async fn workspace_read(computer: &dyn AgentComputer, args: &Value) -> Result<Value, ToolError> {
    let path = required_str(args, "path")?;
    let bytes = computer
        .read_file(path)
        .await
        .map_err(ToolError::ComputerNotReady)?;
    let content = String::from_utf8_lossy(&bytes).into_owned();
    Ok(json!({
        "ok": true,
        "path": path,
        "content": content,
        "bytes": content.len()
    }))
}

async fn workspace_write(computer: &dyn AgentComputer, args: &Value) -> Result<Value, ToolError> {
    let path = required_str(args, "path")?;
    let content = required_str(args, "content")?;
    computer
        .write_file(path, content.as_bytes())
        .await
        .map_err(ToolError::ComputerNotReady)?;
    Ok(json!({
        "ok": true,
        "path": path,
        "bytesWritten": content.len()
    }))
}

async fn workspace_exec(computer: &dyn AgentComputer, args: &Value) -> Result<Value, ToolError> {
    let command = required_str(args, "command")?;
    let response = computer
        .exec(command)
        .await
        .map_err(ToolError::ComputerNotReady)?;
    Ok(json!({
        "ok": response.ok,
        "stdout": response.stdout,
        "stderr": response.stderr,
        "exitCode": response.exit_code
    }))
}

fn map_approval_error(err: ApprovalError) -> ToolError {
    match err {
        ApprovalError::Cancelled => ToolError::Cancelled,
        ApprovalError::Denied { reason } => ToolError::Denied(reason),
        ApprovalError::TimedOut => ToolError::Denied("approval timed out".into()),
        ApprovalError::Internal(detail) => ToolError::MalformedArguments(detail),
    }
}

fn validate_tool_argument_limits(name: &str, args: &Value) -> Result<(), ToolError> {
    if name == "workspace_exec" {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if command.len() > MAX_EXEC_COMMAND_CHARS {
            return Err(ToolError::MalformedArguments(format!(
                "command exceeds {MAX_EXEC_COMMAND_CHARS} characters"
            )));
        }
    }
    Ok(())
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ToolError::MalformedArguments(format!("missing or empty `{key}`")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::computer::{ComputerInfo, ExecResult};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct FakeComputer {
        files: Mutex<HashMap<String, Vec<u8>>>,
    }

    impl FakeComputer {
        fn new() -> Self {
            Self {
                files: Mutex::new(HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl AgentComputer for FakeComputer {
        async fn ensure_ready(&self) -> Result<ComputerInfo, ComputerError> {
            Ok(ComputerInfo {
                ready: true,
                protocol_version: 1,
                detail: None,
            })
        }

        async fn list_dir(
            &self,
            _path: &str,
        ) -> Result<Vec<crate::computer::WorkspaceEntry>, ComputerError> {
            Ok(vec![])
        }

        async fn read_file(&self, path: &str) -> Result<Vec<u8>, ComputerError> {
            let map = self.files.lock().unwrap();
            map.get(path)
                .cloned()
                .ok_or_else(|| ComputerError::ExecutionFailed("not found".into()))
        }

        async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), ComputerError> {
            self.files
                .lock()
                .unwrap()
                .insert(path.to_string(), data.to_vec());
            Ok(())
        }

        async fn exec(&self, _command: &str) -> Result<ExecResult, ComputerError> {
            Ok(ExecResult {
                ok: true,
                stdout: "ok".into(),
                stderr: String::new(),
                exit_code: 0,
            })
        }
    }

    fn test_run() -> ToolRunContext {
        ToolRunContext {
            run_id: "run-1".into(),
            request_id: "req-1".into(),
            owner_id: "owner".into(),
            bot_id: "bot".into(),
            computer_id: "comp".into(),
        }
    }

    #[tokio::test]
    async fn dispatch_write_does_not_require_vm() {
        let computer = FakeComputer::new();
        let cancel = AtomicBool::new(false);
        let result = dispatch_tool(
            &computer,
            "workspace_write",
            r#"{"path":"/workspace/a.txt","content":"hi"}"#,
            &cancel,
            &test_run(),
        )
        .await
        .expect("write");
        assert_eq!(result.get("ok"), Some(&json!(true)));
        assert_eq!(
            computer.read_file("/workspace/a.txt").await.unwrap(),
            b"hi".to_vec()
        );
    }

    #[tokio::test]
    async fn cancellation_checked_early() {
        let computer = FakeComputer::new();
        let cancel = AtomicBool::new(true);
        let err = dispatch_tool(
            &computer,
            "workspace_read",
            r#"{"path":"/workspace/x"}"#,
            &cancel,
            &test_run(),
        )
        .await
        .unwrap_err();
        assert_eq!(err, ToolError::Cancelled);
    }
}
