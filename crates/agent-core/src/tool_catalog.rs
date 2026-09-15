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
}
