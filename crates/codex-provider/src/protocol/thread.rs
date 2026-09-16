use std::path::PathBuf;

use agent_core::ALL_AGENT_TOOL_NAMES;
use serde_json::{json, Value};

use crate::error::CodexProviderError;

pub const MCP_SERVER_NAME: &str = "elsewhere";

pub const ELSEWHERE_ENABLED_MCP_TOOLS: &[&str] = ALL_AGENT_TOOL_NAMES;

pub const ELSEWHERE_OMIT_MCP_TOOL_EXPOSURES: &[&str] = &["deferred", "code_mode"];

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

/// Infrastructure-only Codex thread: no MCP, no host tools, read-only sandbox.
#[derive(Debug, Clone)]
pub struct ToollessThreadConfig {
    pub cwd: PathBuf,
    pub model: String,
    pub developer_instructions: Option<String>,
}

impl ToollessThreadConfig {
    pub fn validate(&self) -> Result<(), CodexProviderError> {
        if !self.cwd.is_absolute() {
            return Err(CodexProviderError::Config(
                "tool-less Codex cwd must be absolute".into(),
            ));
        }
        Ok(())
    }
}

pub fn build_toolless_thread_start_params(
    config: &ToollessThreadConfig,
) -> Result<Value, CodexProviderError> {
    config.validate()?;
    let mut features = json!({});
    for (key, enabled) in REQUIRED_FEATURE_DISABLES {
        features[key] = json!(enabled);
    }
    let mut payload = json!({
        "model": config.model,
        "cwd": config.cwd.to_string_lossy(),
        "sandbox": "read-only",
        "approvalPolicy": "never",
        "config": {
            "features": features
        }
    });
    if let Some(dev) = &config.developer_instructions {
        payload["developerInstructions"] = json!(dev);
    }
    Ok(payload)
}

pub fn build_elsewhere_thread_start_params(
    config: &ElsewhereThreadConfig,
) -> Result<Value, CodexProviderError> {
    config.validate_restrictions()?;

    let mut features = json!({});
    for (key, enabled) in REQUIRED_FEATURE_DISABLES {
        features[key] = json!(enabled);
    }

    let enabled_tools: Vec<&str> = ELSEWHERE_ENABLED_MCP_TOOLS.to_vec();
    let omit_tools_from: Vec<&str> = ELSEWHERE_OMIT_MCP_TOOL_EXPOSURES.to_vec();

    let mcp_servers = json!({
        MCP_SERVER_NAME: {
            "url": config.mcp_url,
            "bearer_token_env_var": config.bearer_env_var,
            "required": true,
            "startup_timeout_sec": 10,
            "enabled_tools": enabled_tools,
            "omit_tools_from": omit_tools_from,
            "default_tools_approval_mode": "approve",
            "tools": {
                "workspace_list": { "approval_mode": "approve" },
                "workspace_read": { "approval_mode": "approve" },
                "workspace_write": { "approval_mode": "approve" },
                "workspace_exec": { "approval_mode": "approve" },
                "browser_navigate": { "approval_mode": "approve" },
                "browser_snapshot": { "approval_mode": "approve" },
                "browser_click": { "approval_mode": "approve" },
                "browser_type": { "approval_mode": "approve" },
                "browser_screenshot": { "approval_mode": "approve" },
                "browser_download": { "approval_mode": "approve" },
                "browser_request_human": { "approval_mode": "approve" },
                "bot_list": { "approval_mode": "approve" },
                "bot_delegate": { "approval_mode": "approve" },
                "run_subagent": { "approval_mode": "approve" },
                "github_list_repositories": { "approval_mode": "approve" },
                "github_search_repositories": { "approval_mode": "approve" },
                "github_get_repository": { "approval_mode": "approve" },
                "github_get_file_contents": { "approval_mode": "approve" },
                "github_list_issues": { "approval_mode": "approve" },
                "github_get_issue": { "approval_mode": "approve" },
                "github_list_pull_requests": { "approval_mode": "approve" },
                "github_get_pull_request": { "approval_mode": "approve" }
            }
        }
    });

    let mut payload = json!({
        "model": config.model,
        "cwd": config.cwd.to_string_lossy(),
        "sandbox": "read-only",
        // Phase 3B.2: Codex-side auto-approve so subscription E2E can run unattended.
        // Phase 3C+ must enforce user-facing approvals in Elsewhere before AgentComputer
        // dispatch (especially workspace_write / workspace_exec), not via Codex prompts.
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

pub fn build_elsewhere_thread_resume_params(
    thread_id: &str,
    config: &ElsewhereThreadConfig,
) -> Result<Value, CodexProviderError> {
    let mut payload = build_elsewhere_thread_start_params(config)?;
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("threadId".into(), json!(thread_id));
    }
    Ok(payload)
}

pub fn parse_thread_resume_response(value: Value) -> Result<String, CodexProviderError> {
    parse_thread_start_response(value)
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

pub fn assert_elsewhere_mcp_direct_exposure(params: &Value) -> Result<(), CodexProviderError> {
    assert_host_tools_disabled(params)?;

    if params.get("sandbox").and_then(|v| v.as_str()) != Some("read-only") {
        return Err(CodexProviderError::Config(
            "Elsewhere thread must use sandbox=read-only".into(),
        ));
    }
    if params.get("approvalPolicy").and_then(|v| v.as_str()) != Some("never") {
        return Err(CodexProviderError::Config(
            "Elsewhere thread must use approvalPolicy=never".into(),
        ));
    }

    let mcp_servers = params
        .get("config")
        .and_then(|c| c.get("mcp_servers"))
        .and_then(|v| v.as_object())
        .ok_or_else(|| {
            CodexProviderError::Config("missing config.mcp_servers for Elsewhere thread".into())
        })?;
    let elsewhere = mcp_servers.get(MCP_SERVER_NAME).ok_or_else(|| {
        CodexProviderError::Config(format!("missing mcp_servers.{MCP_SERVER_NAME}"))
    })?;

    if elsewhere.get("required").and_then(|v| v.as_bool()) != Some(true) {
        return Err(CodexProviderError::Config(
            "elsewhere MCP server must be required".into(),
        ));
    }

    let enabled = elsewhere
        .get("enabled_tools")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            CodexProviderError::Config("elsewhere MCP server missing enabled_tools".into())
        })?;
    let enabled_names: Vec<String> = enabled
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    let expected: Vec<String> = ELSEWHERE_ENABLED_MCP_TOOLS
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    if enabled_names != expected {
        return Err(CodexProviderError::Config(format!(
            "elsewhere enabled_tools must be exactly {expected:?}, got {enabled_names:?}"
        )));
    }

    let omit = elsewhere
        .get("omit_tools_from")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            CodexProviderError::Config("elsewhere MCP server missing omit_tools_from".into())
        })?;
    let omit_values: Vec<String> = omit
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    for required in ELSEWHERE_OMIT_MCP_TOOL_EXPOSURES {
        if !omit_values.iter().any(|v| v == required) {
            return Err(CodexProviderError::Config(format!(
                "elsewhere omit_tools_from must include {required}"
            )));
        }
    }

    if elsewhere
        .get("default_tools_approval_mode")
        .and_then(|v| v.as_str())
        != Some("approve")
    {
        return Err(CodexProviderError::Config(
            "elsewhere MCP server must set default_tools_approval_mode=approve for approvalPolicy=never".into(),
        ));
    }

    let tools = elsewhere
        .get("tools")
        .and_then(|v| v.as_object())
        .ok_or_else(|| {
            CodexProviderError::Config("elsewhere MCP server missing tools map".into())
        })?;
    for tool_name in ELSEWHERE_ENABLED_MCP_TOOLS {
        let tool_cfg = tools.get(*tool_name).ok_or_else(|| {
            CodexProviderError::Config(format!("elsewhere MCP tools missing entry for {tool_name}"))
        })?;
        if tool_cfg.get("approval_mode").and_then(|v| v.as_str()) != Some("approve") {
            return Err(CodexProviderError::Config(format!(
                "elsewhere MCP tool {tool_name} must set approval_mode=approve"
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> ElsewhereThreadConfig {
        ElsewhereThreadConfig {
            cwd: PathBuf::from("/tmp/elsewhere-empty"),
            mcp_url: "http://127.0.0.1:1234/mcp".into(),
            bearer_env_var: "ELSEWHERE_MCP_TOKEN".into(),
            model: "gpt-5.6-luna".into(),
            base_instructions: None,
            developer_instructions: None,
        }
    }

    #[test]
    fn toolless_thread_config_has_no_mcp_and_disables_host_tools() {
        let config = ToollessThreadConfig {
            cwd: PathBuf::from("/tmp/elsewhere-toolless"),
            model: "gpt-5.6-luna".into(),
            developer_instructions: Some("router only".into()),
        };
        let params = build_toolless_thread_start_params(&config).unwrap();
        assert_host_tools_disabled(&params).unwrap();
        let config_obj = params
            .get("config")
            .and_then(|v| v.as_object())
            .expect("config object");
        assert!(
            !config_obj.contains_key("mcp_servers"),
            "tool-less thread must not configure MCP servers"
        );
        let encoded = params.to_string();
        assert!(
            !encoded.contains("run_subagent"),
            "tool-less helper thread must not expose run_subagent"
        );
        assert!(
            !encoded.contains("mcpServers") && !encoded.contains("mcp_servers"),
            "tool-less helper thread must not configure MCP"
        );
    }

    #[test]
    fn thread_config_disables_host_tools_and_forces_direct_mcp_exposure() {
        let params = build_elsewhere_thread_start_params(&sample_config()).unwrap();
        assert_host_tools_disabled(&params).unwrap();
        assert_elsewhere_mcp_direct_exposure(&params).unwrap();
    }

    #[test]
    fn thread_start_and_resume_carry_bot_identity_in_base_instructions() {
        let identity = "You are \"Designer\", an AI teammate in Elsewhere.\n\nYour assigned role:\nDesign specialist.";
        let config = ElsewhereThreadConfig {
            cwd: PathBuf::from("/tmp/elsewhere-empty"),
            mcp_url: "http://127.0.0.1:1234/mcp".into(),
            bearer_env_var: "ELSEWHERE_MCP_TOKEN".into(),
            model: "gpt-5.6-luna".into(),
            base_instructions: Some(identity.into()),
            developer_instructions: Some("execution policy".into()),
        };
        let start = build_elsewhere_thread_start_params(&config).unwrap();
        let resume = build_elsewhere_thread_resume_params("thread-abc", &config).unwrap();
        assert_eq!(
            start.get("baseInstructions").and_then(|v| v.as_str()),
            Some(identity)
        );
        assert_eq!(
            resume.get("baseInstructions").and_then(|v| v.as_str()),
            Some(identity)
        );
        assert_eq!(
            resume.get("threadId").and_then(|v| v.as_str()),
            Some("thread-abc")
        );
    }
}
