use std::path::Path;
use std::process::Command;

use serde_json::Value;

use crate::error::CodexProviderError;

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
    Ok(())
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

    #[test]
    fn detects_missing_methods() {
        let sample = serde_json::json!({
            "oneOf": [
                { "properties": { "method": { "enum": ["initialize"] } } }
            ]
        });
        let methods = extract_request_methods(&sample);
        assert_eq!(methods, vec!["initialize".to_string()]);
    }
}
