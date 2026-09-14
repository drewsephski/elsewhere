use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::error::CodexProviderError;

pub const MCP_SERVER_NAME: &str = "elsewhere";

const REQUIRED_FEATURE_DISABLES: &[(&str, bool)] = &[
    ("shell_tool", false),
    ("unified_exec", false),
    ("standalone_web_search", false),
];

#[derive(Debug, Clone)]
pub struct ElsewhereThreadConfig {
    pub cwd: PathBuf,
    pub mcp_url: String,
    pub bearer_env_var: String,
    pub model: String,
    pub base_instructions: Option<String>,
    pub developer_instructions: Option<String>,
}

impl ElsewhereThreadConfig {
    pub fn validate_restrictions(&self) -> Result<(), CodexProviderError> {
        if !self.cwd.is_absolute() {
            return Err(CodexProviderError::Config(
                "Elsewhere Codex cwd must be absolute".into(),
            ));
        }
        if !self.mcp_url.starts_with("http://127.0.0.1:") {
            return Err(CodexProviderError::Config(
                "Elsewhere MCP URL must use loopback HTTP".into(),
            ));
        }
        Ok(())
    }
}

pub fn build_elsewhere_thread_start_params(
    config: &ElsewhereThreadConfig,
) -> Result<Value, CodexProviderError> {
    config.validate_restrictions()?;

    let mut features = json!({});
    for (key, enabled) in REQUIRED_FEATURE_DISABLES {
        features[key] = json!(enabled);
    }

    let mcp_servers = json!({
        MCP_SERVER_NAME: {
            "url": config.mcp_url,
            "bearer_token_env_var": config.bearer_env_var,
            "required": true,
            "startup_timeout_sec": 10
        }
    });

    let mut payload = json!({
        "model": config.model,
        "cwd": config.cwd.to_string_lossy(),
        "sandbox": "read-only",
        "approvalPolicy": "never",
        "config": {
            "features": features,
            "mcp_servers": mcp_servers
        }
    });
    if let Some(base) = &config.base_instructions {
        payload["baseInstructions"] = json!(base);
    }
    if let Some(dev) = &config.developer_instructions {
        payload["developerInstructions"] = json!(dev);
    }
    Ok(payload)
}

pub fn parse_thread_start_response(value: Value) -> Result<String, CodexProviderError> {
    value
        .get("threadId")
        .or_else(|| value.get("thread").and_then(|t| t.get("id")))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| CodexProviderError::Protocol("thread/start missing threadId".into()))
}

pub fn parse_list_mcp_status(
    value: Value,
    server_name: Option<&str>,
) -> Result<Vec<String>, CodexProviderError> {
    let mut tool_names = Vec::new();
    let servers = value
        .get("data")
        .or_else(|| value.get("servers"))
        .or_else(|| value.get("mcpServers"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    for server in servers {
        if let Some(expected) = server_name {
            let name = server.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if name != expected {
                continue;
            }
        }
        if let Some(tools) = server.get("tools").and_then(|t| t.as_object()) {
            tool_names.extend(tools.keys().cloned());
        }
        if let Some(tools) = server.get("tools").and_then(|t| t.as_array()) {
            for tool in tools {
                if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
                    tool_names.push(name.to_string());
                }
            }
        }
    }
    tool_names.sort();
    tool_names.dedup();
    Ok(tool_names)
}

pub fn assert_host_tools_disabled(params: &Value) -> Result<(), CodexProviderError> {
    let config = params
        .get("config")
        .ok_or_else(|| CodexProviderError::Config("missing thread config overrides".into()))?;
    let features = config.get("features").ok_or_else(|| {
        CodexProviderError::Config("missing features overrides for Elsewhere Codex thread".into())
    })?;
    for (key, expected) in REQUIRED_FEATURE_DISABLES {
        let actual = features.get(key).and_then(|v| v.as_bool());
        if actual != Some(*expected) {
            return Err(CodexProviderError::Config(format!(
                "required feature override `{key}={expected}` missing or incorrect (got {actual:?})"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_config_disables_host_tools() {
        let params = build_elsewhere_thread_start_params(&ElsewhereThreadConfig {
            cwd: PathBuf::from("/tmp/elsewhere-empty"),
            mcp_url: "http://127.0.0.1:1234/mcp".into(),
            bearer_env_var: "ELSEWHERE_MCP_TOKEN".into(),
            model: "gpt-5.6-luna".into(),
            base_instructions: None,
            developer_instructions: None,
        })
        .unwrap();
        assert_host_tools_disabled(&params).unwrap();
    }
}
