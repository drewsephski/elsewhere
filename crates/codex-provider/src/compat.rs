use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use serde_json::Value;

use crate::error::CodexProviderError;
use crate::process::which_codex_executable;
use crate::protocol::thread::ELSEWHERE_OMIT_MCP_TOOL_EXPOSURES;

pub const REQUIRED_PROTOCOL_METHODS: &[&str] = &[
    "initialize",
    "account/read",
    "account/login/start",
    "account/login/completed",
    "account/rateLimits/read",
    "thread/start",
    "mcpServerStatus/list",
    "turn/start",
    "turn/interrupt",
    "turn/completed",
];

const NOTIFICATION_ONLY_METHODS: &[&str] = &["account/login/completed", "turn/completed"];

static MCP_TOOL_EXPOSURE_OK: OnceLock<Result<(), String>> = OnceLock::new();

pub fn ensure_codex_mcp_tool_exposure_supported() -> Result<(), CodexProviderError> {
    MCP_TOOL_EXPOSURE_OK
        .get_or_init(|| {
            verify_codex_mcp_tool_exposure_support().map_err(|err| err.to_string())
        })
        .as_ref()
        .map(|_| ())
        .map_err(|msg| {
            if msg.contains("unsupported_codex_tool_exposure") {
                CodexProviderError::UnsupportedCodexToolExposure(
                    msg.replace("unsupported_codex_tool_exposure: ", ""),
                )
            } else {
                CodexProviderError::UnsupportedCodexToolExposure(msg.clone())
            }
        })
}

pub fn verify_generated_schema_dir(dir: &Path) -> Result<(), CodexProviderError> {
    let client_request = dir.join("ClientRequest.json");
    let server_notification = dir.join("ServerNotification.json");
    if !client_request.is_file() || !server_notification.is_file() {
        return Err(CodexProviderError::UnsupportedCodexVersion(format!(
            "missing schema bundle files in {}",
            dir.display()
        )));
    }
    let client_text = std::fs::read_to_string(&client_request).map_err(|e| {
        CodexProviderError::UnsupportedCodexVersion(format!("read ClientRequest.json: {e}"))
    })?;
    let client_value: Value = serde_json::from_str(&client_text).map_err(|e| {
        CodexProviderError::UnsupportedCodexVersion(format!("parse ClientRequest.json: {e}"))
    })?;
    let methods = extract_request_methods(&client_value);

    let notification_text = std::fs::read_to_string(&server_notification).map_err(|e| {
        CodexProviderError::UnsupportedCodexVersion(format!("read ServerNotification.json: {e}"))
    })?;
    let notification_value: Value = serde_json::from_str(&notification_text).map_err(|e| {
        CodexProviderError::UnsupportedCodexVersion(format!("parse ServerNotification.json: {e}"))
    })?;
    let notifications = extract_notification_methods(&notification_value);

    let mut missing = Vec::new();
    for required in REQUIRED_PROTOCOL_METHODS {
        if NOTIFICATION_ONLY_METHODS.contains(required) {
            if !has_method(&notifications, required) {
                missing.push(*required);
            }
        } else if !has_method(&methods, required) {
            missing.push(*required);
        }
    }
    if !missing.is_empty() {
        return Err(CodexProviderError::UnsupportedCodexVersion(format!(
            "installed Codex app-server missing methods: {}",
            missing.join(", ")
        )));
    }

    verify_mcp_tool_exposure_in_schema_dir(dir)?;
    Ok(())
}

fn verify_codex_mcp_tool_exposure_support() -> Result<(), CodexProviderError> {
    let executable = which_codex_executable()?;
    let temp = tempfile::tempdir().map_err(|e| {
        CodexProviderError::UnsupportedCodexToolExposure(format!("tempdir: {e}"))
    })?;
    generate_schema_to_dir(temp.path())?;
    verify_mcp_tool_exposure_in_schema_dir(temp.path()).or_else(|schema_err| {
        verify_mcp_tool_exposure_strict_config(&executable).or_else(|strict_err| {
            Err(CodexProviderError::UnsupportedCodexToolExposure(format!(
                "{schema_err}; strict-config probe: {strict_err}"
            )))
        })
    })
}

fn verify_mcp_tool_exposure_in_schema_dir(dir: &Path) -> Result<(), CodexProviderError> {
    let mut saw_enabled_tools = false;
    let mut saw_omit_tools_from = false;
    let mut exposure_enums: Vec<String> = Vec::new();

    for entry in walkdir_json_files(dir)? {
        let text = std::fs::read_to_string(&entry).map_err(|e| {
            CodexProviderError::UnsupportedCodexToolExposure(format!(
                "read {}: {e}",
                entry.display()
            ))
        })?;
        let value: Value = serde_json::from_str(&text).map_err(|e| {
            CodexProviderError::UnsupportedCodexToolExposure(format!(
                "parse {}: {e}",
                entry.display()
            ))
        })?;
        scan_schema_value(&value, &mut saw_enabled_tools, &mut saw_omit_tools_from, &mut exposure_enums);
    }

    if !saw_enabled_tools || !saw_omit_tools_from {
        return Err(CodexProviderError::UnsupportedCodexToolExposure(
            "installed Codex schema missing mcp_servers.*.enabled_tools or omit_tools_from"
                .into(),
        ));
    }

    if !exposure_enums.is_empty() {
        for required in ELSEWHERE_OMIT_MCP_TOOL_EXPOSURES {
            if !exposure_enums.iter().any(|v| v == required) {
                return Err(CodexProviderError::UnsupportedCodexToolExposure(format!(
                    "installed Codex schema missing omit_tools_from exposure value `{required}`"
                )));
            }
        }
    }

    Ok(())
}

fn verify_mcp_tool_exposure_strict_config(executable: &Path) -> Result<(), CodexProviderError> {
    let enabled_probe = r#"mcp_servers.__elsewhere_compat_probe.enabled_tools=["workspace_list"]"#;
    let omit_probe = r#"mcp_servers.__elsewhere_compat_probe.omit_tools_from=["deferred","code_mode"]"#;
    let status = Command::new(executable)
        .args([
            "--strict-config",
            "-c",
            enabled_probe,
            "-c",
            omit_probe,
            "app-server",
            "--help",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| CodexProviderError::UnsupportedCodexToolExposure(e.to_string()))?;
    if !status.success() {
        return Err(CodexProviderError::UnsupportedCodexToolExposure(
            "codex --strict-config rejected mcp_servers enabled_tools/omit_tools_from overrides"
                .into(),
        ));
    }
    Ok(())
}

fn walkdir_json_files(dir: &Path) -> Result<Vec<PathBuf>, CodexProviderError> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let read_dir = std::fs::read_dir(&current).map_err(|e| {
            CodexProviderError::UnsupportedCodexToolExposure(format!(
                "read dir {}: {e}",
                current.display()
            ))
        })?;
        for entry in read_dir {
            let entry = entry.map_err(|e| {
                CodexProviderError::UnsupportedCodexToolExposure(e.to_string())
            })?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
                files.push(path);
            }
        }
    }
    Ok(files)
}

fn scan_schema_value(
    value: &Value,
    saw_enabled_tools: &mut bool,
    saw_omit_tools_from: &mut bool,
    exposure_enums: &mut Vec<String>,
) {
    match value {
        Value::Object(map) => {
            if map.contains_key("enabled_tools") {
                *saw_enabled_tools = true;
            }
            if map.contains_key("omit_tools_from") {
                *saw_omit_tools_from = true;
            }
            for (key, child) in map {
                if key == "enum" {
                    if let Value::Array(items) = child {
                        for item in items {
                            if let Some(s) = item.as_str() {
                                if matches!(s, "deferred" | "code_mode" | "direct") {
                                    if !exposure_enums.iter().any(|v| v == s) {
                                        exposure_enums.push(s.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
                scan_schema_value(child, saw_enabled_tools, saw_omit_tools_from, exposure_enums);
            }
        }
        Value::Array(items) => {
            for item in items {
                scan_schema_value(item, saw_enabled_tools, saw_omit_tools_from, exposure_enums);
            }
        }
        _ => {}
    }
}

pub fn generate_schema_to_dir(out_dir: &Path) -> Result<(), CodexProviderError> {
    let executable = crate::process::which_codex_executable()?;
    let status = Command::new(executable)
        .args([
            "app-server",
            "generate-json-schema",
            "--experimental",
            "--out",
        ])
        .arg(out_dir)
        .status()
        .map_err(|e| CodexProviderError::Process(e.to_string()))?;
    if !status.success() {
        return Err(CodexProviderError::Process(
            "codex app-server generate-json-schema failed".into(),
        ));
    }
    Ok(())
}

fn has_method(haystack: &[String], needle: &str) -> bool {
    haystack.iter().any(|entry| entry == needle)
}

fn extract_notification_methods(server_notification: &Value) -> Vec<String> {
    let mut methods = Vec::new();
    if let Some(one_of) = server_notification.get("oneOf").and_then(|v| v.as_array()) {
        for entry in one_of {
            if let Some(method) = entry
                .get("properties")
                .and_then(|p| p.get("method"))
                .and_then(|m| m.get("enum"))
                .and_then(|e| e.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
            {
                methods.push(method.to_string());
            }
        }
    }
    methods
}

fn extract_request_methods(client_request: &Value) -> Vec<String> {
    let mut methods = Vec::new();
    if let Some(one_of) = client_request.get("oneOf").and_then(|v| v.as_array()) {
        for entry in one_of {
            if let Some(method) = entry
                .get("properties")
                .and_then(|p| p.get("method"))
                .and_then(|m| m.get("enum"))
                .and_then(|e| e.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
            {
                methods.push(method.to_string());
            }
        }
    }
    methods
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_missing_methods() {
        let sample = json!({
            "oneOf": [
                { "properties": { "method": { "enum": ["initialize"] } } }
            ]
        });
        let methods = extract_request_methods(&sample);
        assert_eq!(methods, vec!["initialize".to_string()]);
    }

    #[test]
    fn schema_scan_finds_mcp_exposure_fields() {
        let sample = json!({
            "properties": {
                "enabled_tools": { "type": "array" },
                "omit_tools_from": {
                    "items": {
                        "enum": ["direct", "deferred", "code_mode"]
                    }
                }
            }
        });
        let mut enabled = false;
        let mut omit = false;
        let mut enums = Vec::new();
        scan_schema_value(&sample, &mut enabled, &mut omit, &mut enums);
        assert!(enabled);
        assert!(omit);
        assert!(enums.iter().any(|v| v == "deferred"));
        assert!(enums.iter().any(|v| v == "code_mode"));
    }
}
