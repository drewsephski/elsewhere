use crate::vm::{GuestRequest, GuestResponse, VirtualMachineManager};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const MAX_AGENT_TOOL_STEPS: usize = 25;
const GUEST_READY_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolError {
    VmNotProvisioned,
    VmBootFailed(String),
    GuestUnavailable(String),
    MalformedArguments(String),
    SandboxRejected(String),
    ExecutionFailed(String),
    Cancelled,
}

impl ToolError {
    pub fn code(&self) -> &'static str {
        match self {
            ToolError::VmNotProvisioned => "vm_not_provisioned",
            ToolError::VmBootFailed(_) => "vm_boot_failed",
            ToolError::GuestUnavailable(_) => "guest_unavailable",
            ToolError::MalformedArguments(_) => "malformed_tool_arguments",
            ToolError::SandboxRejected(_) => "tool_rejected_by_sandbox",
            ToolError::ExecutionFailed(_) => "tool_execution_failed",
            ToolError::Cancelled => "cancelled",
        }
    }

    pub fn message(&self) -> String {
        match self {
            ToolError::VmNotProvisioned => {
                "Local computer is not provisioned. Open VM diagnostics and provision once.".into()
            }
            ToolError::VmBootFailed(d) => format!("VM boot failed: {d}"),
            ToolError::GuestUnavailable(d) => format!("Guest agent unavailable: {d}"),
            ToolError::MalformedArguments(d) => d.clone(),
            ToolError::SandboxRejected(d) => d.clone(),
            ToolError::ExecutionFailed(d) => d.clone(),
            ToolError::Cancelled => "Agent run cancelled".into(),
        }
    }
}

pub fn openai_tool_definitions() -> Vec<Value> {
    vec![
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
    ]
}

pub fn ensure_computer_ready(vm: &VirtualMachineManager) -> Result<(), ToolError> {
    if !vm.layout().vm_config_path().exists() {
        return Err(ToolError::VmNotProvisioned);
    }
    vm.ensure_guest_ready(GUEST_READY_TIMEOUT).map_err(|err| {
        if err.contains("not provisioned") {
            ToolError::VmNotProvisioned
        } else if err.contains("boot") || err.contains("start") {
            ToolError::VmBootFailed(err)
        } else {
            ToolError::GuestUnavailable(err)
        }
    })?;
    Ok(())
}

pub fn dispatch_tool(
    vm: &VirtualMachineManager,
    name: &str,
    arguments: &str,
    cancel: &AtomicBool,
) -> Result<Value, ToolError> {
    if cancel.load(Ordering::Relaxed) {
        return Err(ToolError::Cancelled);
    }

    let args: Value = serde_json::from_str(arguments).map_err(|e| {
        ToolError::MalformedArguments(format!("invalid JSON arguments: {e}"))
    })?;

    ensure_computer_ready(vm)?;

    if cancel.load(Ordering::Relaxed) {
        return Err(ToolError::Cancelled);
    }

    let started = Instant::now();
    let result = match name {
        "workspace_list" => workspace_list(vm, &args),
        "workspace_read" => workspace_read(vm, &args),
        "workspace_write" => workspace_write(vm, &args),
        "workspace_exec" => workspace_exec(vm, &args),
        other => Err(ToolError::MalformedArguments(format!("unknown tool: {other}"))),
    }?;

    let mut envelope = result;
    if let Some(obj) = envelope.as_object_mut() {
        obj.insert("durationMs".into(), json!(started.elapsed().as_millis()));
    }
    Ok(envelope)
}

fn workspace_list(vm: &VirtualMachineManager, args: &Value) -> Result<Value, ToolError> {
    let path = required_str(args, "path")?;
    let response = guest_call(vm, "list_dir", json!({ "path": path }))?;
    let entries: Value = response
        .stdout
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(json!([]));
    Ok(json!({
        "ok": true,
        "path": path,
        "entries": entries
    }))
}

fn workspace_read(vm: &VirtualMachineManager, args: &Value) -> Result<Value, ToolError> {
    let path = required_str(args, "path")?;
    let response = guest_call(vm, "read_file", json!({ "path": path }))?;
    let content = response.stdout.unwrap_or_default();
    Ok(json!({
        "ok": true,
        "path": path,
        "content": content,
        "bytes": content.len()
    }))
}

fn workspace_write(vm: &VirtualMachineManager, args: &Value) -> Result<Value, ToolError> {
    let path = required_str(args, "path")?;
    let content = required_str(args, "content")?;
    guest_call(
        vm,
        "write_file",
        json!({ "path": path, "content": content }),
    )?;
    Ok(json!({
        "ok": true,
        "path": path,
        "bytesWritten": content.len()
    }))
}

fn workspace_exec(vm: &VirtualMachineManager, args: &Value) -> Result<Value, ToolError> {
    let command = required_str(args, "command")?;
    let response = guest_call(vm, "exec", json!({ "command": command }))?;
    Ok(json!({
        "ok": response.ok,
        "stdout": response.stdout.unwrap_or_default(),
        "stderr": response.stderr.unwrap_or_default(),
        "exitCode": response.exit_code.unwrap_or(-1)
    }))
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ToolError::MalformedArguments(format!("missing or empty `{key}`")))
}

fn guest_call(
    vm: &VirtualMachineManager,
    method: &str,
    params: Value,
) -> Result<GuestResponse, ToolError> {
    let response = vm
        .guest_request(GuestRequest {
            id: uuid::Uuid::new_v4().to_string(),
            method: method.into(),
            params,
        })
        .map_err(ToolError::ExecutionFailed)?;

    if !response.ok {
        let message = response
            .error
            .unwrap_or_else(|| "guest request failed".into());
        if message.contains("sandbox") || message.contains("blocked") {
            return Err(ToolError::SandboxRejected(message));
        }
        return Err(ToolError::ExecutionFailed(message));
    }

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_malformed_json_arguments() {
        let vm = VirtualMachineManager::new(std::path::Path::new("/tmp/gptbot-tool-test-missing"));
        let cancel = AtomicBool::new(false);
        let err = dispatch_tool(&vm, "workspace_read", "{not json", &cancel).unwrap_err();
        assert_eq!(err.code(), "malformed_tool_arguments");
    }

    #[test]
    fn rejects_unknown_tool_name() {
        let vm = VirtualMachineManager::new(std::path::Path::new("/tmp/gptbot-tool-test-missing"));
        let cancel = AtomicBool::new(false);
        let err = dispatch_tool(&vm, "host_shell", "{}", &cancel).unwrap_err();
        assert!(matches!(
            err,
            ToolError::MalformedArguments(_) | ToolError::VmNotProvisioned
        ));
    }

    #[test]
    fn cancellation_checked_before_provision() {
        let vm = VirtualMachineManager::new(std::path::Path::new("/tmp/gptbot-tool-test-missing"));
        let cancel = AtomicBool::new(true);
        let err = dispatch_tool(&vm, "workspace_read", r#"{"path":"/workspace/x"}"#, &cancel)
            .unwrap_err();
        assert_eq!(err, ToolError::Cancelled);
    }
}
