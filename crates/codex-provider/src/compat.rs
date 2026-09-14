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
        .get_or_init(|| verify_codex_mcp_tool_exposure_support().map_err(|err| err.to_string()))
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

/// Verify app-server protocol methods. MCP configuration is a separate CLI schema;
/// callers must also run `ensure_codex_mcp_tool_exposure_supported`.
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

    // The app-server protocol bundle does not include the CLI configuration schema.
    // Check installed MCP configuration support separately via
    // ensure_codex_mcp_tool_exposure_supported.
    Ok(())
}

fn verify_codex_mcp_tool_exposure_support() -> Result<(), CodexProviderError> {
    let executable = which_codex_executable()?;
    let temp = tempfile::tempdir()
        .map_err(|e| CodexProviderError::UnsupportedCodexToolExposure(format!("tempdir: {e}")))?;
    generate_schema_to_dir(temp.path())?;
    verify_mcp_tool_exposure_in_schema_dir(temp.path()).or_else(|schema_err| {
        verify_mcp_tool_exposure_config(&executable).or_else(|strict_err| {
            Err(CodexProviderError::UnsupportedCodexToolExposure(format!(
                "{schema_err}; configuration probe: {strict_err}"
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
        scan_schema_value(
            &value,
            &mut saw_enabled_tools,
            &mut saw_omit_tools_from,
            &mut exposure_enums,
        );
    }

    if !saw_enabled_tools || !saw_omit_tools_from {
        return Err(CodexProviderError::UnsupportedCodexToolExposure(
            "installed Codex schema missing mcp_servers.*.enabled_tools or omit_tools_from".into(),
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

// `app-server --help` exits before parsing configuration, even with --strict-config.
// Use the actual MCP config loader in an empty profile, without contacting a server.
fn verify_mcp_tool_exposure_config(executable: &Path) -> Result<(), CodexProviderError> {
    let profile = tempfile::tempdir()
        .map_err(|e| CodexProviderError::UnsupportedCodexToolExposure(e.to_string()))?;
    let valid = probe_mcp_config(executable, profile.path(), "deferred", "code_mode")?;
    if !valid.status.success() {
        return Err(CodexProviderError::UnsupportedCodexToolExposure(
            "Codex rejected the required MCP configuration".into(),
        ));
    }
    let parsed: Value = serde_json::from_slice(&valid.stdout).map_err(|_| {
        CodexProviderError::UnsupportedCodexToolExposure(
            "Codex returned invalid MCP configuration JSON".into(),
        )
    })?;
    if parsed.get("enabled_tools") != Some(&serde_json::json!(["workspace_list"])) {
        return Err(CodexProviderError::UnsupportedCodexToolExposure(
            "Codex did not preserve the MCP tool allow-list".into(),
        ));
    }
    // mcp/get does not return omit_tools_from. A negative control proves that the
    // installed parser recognizes and validates its enum rather than ignoring it.
    let invalid = probe_mcp_config(
        executable,
        profile.path(),
        "elsewhere_invalid_surface",
        "code_mode",
    )?;
    let error = String::from_utf8_lossy(&invalid.stderr);
    if invalid.status.success()
        || !error.contains("elsewhere_invalid_surface")
        || !error.contains("omit_tools_from")
        || !error.contains("unknown variant")
    {
        return Err(CodexProviderError::UnsupportedCodexToolExposure(
            "Codex did not reject the invalid MCP tool exposure negative control".into(),
        ));
    }
    Ok(())
}

fn probe_mcp_config(
    executable: &Path,
    profile: &Path,
    first_surface: &str,
    second_surface: &str,
) -> Result<std::process::Output, CodexProviderError> {
    let stdout = tempfile::tempfile().map_err(|e| CodexProviderError::Process(e.to_string()))?;
    let stderr = tempfile::tempfile().map_err(|e| CodexProviderError::Process(e.to_string()))?;
    let mut child = Command::new(executable)
        .env("CODEX_HOME", profile)
        .env_remove("OPENAI_API_KEY")
        .current_dir(profile)
        .args([
            "-c", "mcp_servers.__elsewhere_compat_probe.url=\"http://127.0.0.1:9\"",
            "-c", "mcp_servers.__elsewhere_compat_probe.enabled_tools=[\"workspace_list\"]",
            "-c", &format!("mcp_servers.__elsewhere_compat_probe.omit_tools_from=[\"{first_surface}\",\"{second_surface}\"]"),
            "mcp", "get", "__elsewhere_compat_probe", "--json",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(stdout.try_clone().map_err(|e| CodexProviderError::Process(e.to_string()))?)
        .stderr(stderr.try_clone().map_err(|e| CodexProviderError::Process(e.to_string()))?)
        .spawn().map_err(|e| CodexProviderError::Process(e.to_string()))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(CodexProviderError::UnsupportedCodexToolExposure(format!(
                    "MCP configuration probe did not finish: {result:?}"
                )));
            }
        }
    };
    fn read_output(mut file: std::fs::File) -> std::io::Result<Vec<u8>> {
        use std::io::{Read, Seek};
        file.rewind()?;
        let mut bytes = Vec::new();
        file.take(64 * 1024).read_to_end(&mut bytes)?;
        Ok(bytes)
    }
    Ok(std::process::Output {
        status,
        stdout: read_output(stdout).map_err(|e| CodexProviderError::Process(e.to_string()))?,
        stderr: read_output(stderr).map_err(|e| CodexProviderError::Process(e.to_string()))?,
    })
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
            let entry = entry
                .map_err(|e| CodexProviderError::UnsupportedCodexToolExposure(e.to_string()))?;
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
                scan_schema_value(
                    child,
                    saw_enabled_tools,
                    saw_omit_tools_from,
                    exposure_enums,
                );
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

    #[cfg(unix)]
    #[test]
    fn configuration_probe_rejects_ignored_or_missing_restrictions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let executable = dir.path().join("codex");
        for (preserve_allowlist, reject_invalid, expected) in [
            (true, true, true),
            (true, false, false),
            (false, true, false),
        ] {
            let control = if reject_invalid {
                r#"case "$*" in *elsewhere_invalid_surface*)
                    echo 'unknown variant elsewhere_invalid_surface in omit_tools_from' >&2
                    exit 1;; esac"#
            } else {
                ""
            };
            let output = if preserve_allowlist {
                r#"{"enabled_tools":["workspace_list"]}"#
            } else {
                "{}"
            };
            std::fs::write(
                &executable,
                format!("#!/bin/sh\n{control}\necho '{output}'\n"),
            )
            .unwrap();
            std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
            assert_eq!(
                verify_mcp_tool_exposure_config(&executable).is_ok(),
                expected
            );
        }
    }

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
