//! Canonical Elsewhere computer tool names (workspace + browser).

pub const WORKSPACE_TOOL_NAMES: &[&str] = &[
    "workspace_list",
    "workspace_read",
    "workspace_write",
    "workspace_exec",
];

pub const BROWSER_TOOL_NAMES: &[&str] = &[
    "browser_navigate",
    "browser_snapshot",
    "browser_click",
    "browser_type",
    "browser_screenshot",
    "browser_download",
];

pub const COLLABORATION_TOOL_NAMES: &[&str] = &["bot_list", "bot_delegate"];

pub const CONNECTOR_TOOL_NAMES: &[&str] = &[
    "github_list_repositories",
    "github_search_repositories",
    "github_get_repository",
    "github_get_file_contents",
    "github_list_issues",
    "github_get_issue",
    "github_list_pull_requests",
    "github_get_pull_request",
];

pub const ALL_COMPUTER_TOOL_NAMES: &[&str] = &[
    "workspace_list",
    "workspace_read",
    "workspace_write",
    "workspace_exec",
    "browser_navigate",
    "browser_snapshot",
    "browser_click",
    "browser_type",
    "browser_screenshot",
    "browser_download",
];

pub fn is_browser_tool(name: &str) -> bool {
    BROWSER_TOOL_NAMES.contains(&name)
}

/// Browser tools that mutate the page/session (not snapshot reads).
pub fn is_browser_mutation_tool(name: &str) -> bool {
    is_browser_tool(name) && name != "browser_snapshot"
}

pub fn is_collaboration_tool(name: &str) -> bool {
    COLLABORATION_TOOL_NAMES.contains(&name)
}

pub fn is_connector_tool(name: &str) -> bool {
    CONNECTOR_TOOL_NAMES.contains(&name)
}

/// Mutation tools that owners may persistently allow, ask, or deny.
/// Human-only intervention, credentials, and other hard constraints are excluded.
pub const POLICY_OVERRIDABLE_TOOL_NAMES: &[&str] = &[
    "workspace_write",
    "workspace_exec",
    "browser_navigate",
    "browser_click",
    "browser_type",
    "browser_screenshot",
    "browser_download",
    "bot_delegate",
];

/// Agent tools that must never be skipped by a user-configurable Allow policy.
pub const POLICY_NON_OVERRIDABLE_TOOL_NAMES: &[&str] = &["browser_request_human"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyActionGroup {
    Files,
    Terminal,
    Browser,
    Delegation,
    ConnectedApps,
}

impl PolicyActionGroup {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Files => "files",
            Self::Terminal => "terminal",
            Self::Browser => "browser",
            Self::Delegation => "delegation",
            Self::ConnectedApps => "connected_apps",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Files => "Files",
            Self::Terminal => "Terminal",
            Self::Browser => "Browser",
            Self::Delegation => "Delegation",
            Self::ConnectedApps => "Connected apps",
        }
    }
}

pub fn is_known_agent_tool(name: &str) -> bool {
    ALL_AGENT_TOOL_NAMES.contains(&name)
}

pub fn is_policy_overridable_tool(name: &str) -> bool {
    POLICY_OVERRIDABLE_TOOL_NAMES.contains(&name)
}

pub fn is_policy_non_overridable_tool(name: &str) -> bool {
    POLICY_NON_OVERRIDABLE_TOOL_NAMES.contains(&name)
}

pub fn policy_action_group(name: &str) -> Option<PolicyActionGroup> {
    match name {
        "workspace_write" => Some(PolicyActionGroup::Files),
        "workspace_exec" => Some(PolicyActionGroup::Terminal),
        "browser_navigate" | "browser_click" | "browser_type" | "browser_screenshot"
        | "browser_download" => Some(PolicyActionGroup::Browser),
        "bot_delegate" => Some(PolicyActionGroup::Delegation),
        name if is_connector_tool(name) => Some(PolicyActionGroup::ConnectedApps),
        _ => None,
    }
}

pub fn policy_action_label(name: &str) -> &'static str {
    match name {
        "workspace_write" => "Write files",
        "workspace_exec" => "Run commands",
        "browser_navigate" => "Open pages",
        "browser_click" => "Click",
        "browser_type" => "Type",
        "browser_screenshot" => "Take screenshots",
        "browser_download" => "Download files",
        "bot_delegate" => "Hand off work",
        _ => "This action",
    }
}

pub fn policy_denied_message(name: &str) -> String {
    match name {
        "workspace_write" => "This Bot is not allowed to write files.".into(),
        "workspace_exec" => "This Bot is not allowed to run terminal commands.".into(),
        "browser_navigate" => "This Bot is not allowed to open web pages.".into(),
        "browser_click" => "This Bot is not allowed to click in the browser.".into(),
        "browser_type" => "This Bot is not allowed to type in the browser.".into(),
        "browser_screenshot" => "This Bot is not allowed to take screenshots.".into(),
        "browser_download" => "This Bot is not allowed to download files.".into(),
        "bot_delegate" => "This Bot is not allowed to hand work to another Bot.".into(),
        other => format!("This Bot is not allowed to use {other}."),
    }
}

pub const ALL_AGENT_TOOL_NAMES: &[&str] = &[
    "workspace_list",
    "workspace_read",
    "workspace_write",
    "workspace_exec",
    "browser_navigate",
    "browser_snapshot",
    "browser_click",
    "browser_type",
    "browser_screenshot",
    "browser_download",
    "browser_request_human",
    "bot_list",
    "bot_delegate",
    "github_list_repositories",
    "github_search_repositories",
    "github_get_repository",
    "github_get_file_contents",
    "github_list_issues",
    "github_get_issue",
    "github_list_pull_requests",
    "github_get_pull_request",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_agent_tools_include_browser_request_human() {
        assert!(ALL_AGENT_TOOL_NAMES.contains(&"browser_request_human"));
    }

    #[test]
    fn overridable_tools_are_catalogued_and_exclude_hard_boundaries() {
        for name in POLICY_OVERRIDABLE_TOOL_NAMES {
            assert!(is_known_agent_tool(name), "{name}");
            assert!(is_policy_overridable_tool(name), "{name}");
            assert!(!is_policy_non_overridable_tool(name), "{name}");
            assert!(policy_action_group(name).is_some(), "{name}");
        }
        assert!(!is_policy_overridable_tool("browser_request_human"));
        assert!(is_policy_non_overridable_tool("browser_request_human"));
        assert!(!is_policy_overridable_tool("workspace_read"));
        assert!(!is_policy_overridable_tool("github_list_repositories"));
        assert!(!is_policy_overridable_tool("not_a_tool"));
    }
}
