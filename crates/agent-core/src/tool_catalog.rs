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

pub const SUBAGENT_TOOL_NAMES: &[&str] = &["run_subagent"];

pub const MEMORY_TOOL_NAMES: &[&str] = &["recall_memory", "remember", "forget_memory"];

pub const ROUTINE_TOOL_NAMES: &[&str] = &[
    "routine_list",
    "routine_create",
    "routine_pause",
    "routine_resume",
];

pub const SKILL_TOOL_NAMES: &[&str] = &[
    "skill_list",
    "skill_save_recent_work",
    "skill_attach",
    "skill_detach",
];

pub const ATTACHMENT_TOOL_NAMES: &[&str] = &["attachment_list", "attachment_read"];

pub const USER_QUESTION_TOOL_NAMES: &[&str] = &["ask_user"];

pub const GITHUB_CODING_TOOL_NAMES: &[&str] = &[
    "github_open_repository",
    "github_run_check",
    "github_review_publish",
    "github_publish_pull_request",
];

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

pub const CONNECTED_APPS_TOOL_NAMES: &[&str] = &[
    "connected_apps_search_tools",
    "connected_apps_load_tool",
    "connected_apps_execute_tool",
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

pub fn is_subagent_tool(name: &str) -> bool {
    SUBAGENT_TOOL_NAMES.contains(&name)
}

pub fn is_memory_tool(name: &str) -> bool {
    MEMORY_TOOL_NAMES.contains(&name)
}

pub fn is_routine_tool(name: &str) -> bool {
    ROUTINE_TOOL_NAMES.contains(&name)
}

pub fn is_skill_tool(name: &str) -> bool {
    SKILL_TOOL_NAMES.contains(&name)
}

pub fn is_attachment_tool(name: &str) -> bool {
    ATTACHMENT_TOOL_NAMES.contains(&name)
}

pub fn is_user_question_tool(name: &str) -> bool {
    USER_QUESTION_TOOL_NAMES.contains(&name)
}

pub fn is_github_connector_tool(name: &str) -> bool {
    CONNECTOR_TOOL_NAMES.contains(&name)
}

pub fn is_github_coding_tool(name: &str) -> bool {
    GITHUB_CODING_TOOL_NAMES.contains(&name)
}

pub fn is_connected_apps_tool(name: &str) -> bool {
    CONNECTED_APPS_TOOL_NAMES.contains(&name)
}

pub fn is_connector_tool(name: &str) -> bool {
    is_github_connector_tool(name) || is_connected_apps_tool(name)
}

#[allow(dead_code)]
pub fn is_github_coding_dispatch_tool(name: &str) -> bool {
    is_github_coding_tool(name)
}

pub fn is_connected_apps_execute_tool(name: &str) -> bool {
    name == "connected_apps_execute_tool"
}

/// Mutation tools that owners may persistently allow, ask, or deny.
/// Human-only intervention, credentials, and other hard constraints are excluded.
pub const POLICY_OVERRIDABLE_TOOL_NAMES: &[&str] = &[
    "workspace_write",
    "workspace_exec",
    "github_run_check",
    "browser_navigate",
    "browser_click",
    "browser_type",
    "browser_screenshot",
    "browser_download",
    "bot_delegate",
    "run_subagent",
    "remember",
    "forget_memory",
    "routine_create",
    "routine_pause",
    "routine_resume",
    "skill_save_recent_work",
    "skill_attach",
    "skill_detach",
    "github_publish_pull_request",
];

/// Agent tools that must never be skipped by a user-configurable Allow policy.
pub const POLICY_NON_OVERRIDABLE_TOOL_NAMES: &[&str] = &[
    "browser_request_human",
    "ask_user",
    "attachment_list",
    "attachment_read",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyActionGroup {
    Files,
    Terminal,
    Browser,
    Delegation,
    ConnectedApps,
    Memory,
    Routines,
    Skills,
}

impl PolicyActionGroup {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Files => "files",
            Self::Terminal => "terminal",
            Self::Browser => "browser",
            Self::Delegation => "delegation",
            Self::ConnectedApps => "connected_apps",
            Self::Memory => "memory",
            Self::Routines => "routines",
            Self::Skills => "skills",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Files => "Files",
            Self::Terminal => "Terminal",
            Self::Browser => "Browser",
            Self::Delegation => "Delegation",
            Self::ConnectedApps => "Connected apps",
            Self::Memory => "Memory",
            Self::Routines => "Routines",
            Self::Skills => "Skills",
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
        "workspace_exec" | "github_run_check" => Some(PolicyActionGroup::Terminal),
        "browser_navigate" | "browser_click" | "browser_type" | "browser_screenshot"
        | "browser_download" => Some(PolicyActionGroup::Browser),
        "bot_delegate" | "run_subagent" => Some(PolicyActionGroup::Delegation),
        "remember" | "forget_memory" => Some(PolicyActionGroup::Memory),
        "routine_create" | "routine_pause" | "routine_resume" => Some(PolicyActionGroup::Routines),
        "skill_save_recent_work" | "skill_attach" | "skill_detach" => {
            Some(PolicyActionGroup::Skills)
        }
        "github_publish_pull_request" => Some(PolicyActionGroup::ConnectedApps),
        name if is_github_connector_tool(name) || is_connected_apps_tool(name) => {
            Some(PolicyActionGroup::ConnectedApps)
        }
        _ => None,
    }
}

pub fn policy_action_label(name: &str) -> &'static str {
    match name {
        "workspace_write" => "Write files",
        "workspace_exec" => "Run commands",
        "github_run_check" => "Run checks",
        "browser_navigate" => "Open pages",
        "browser_click" => "Click",
        "browser_type" => "Type",
        "browser_screenshot" => "Take screenshots",
        "browser_download" => "Download files",
        "bot_delegate" => "Hand off work",
        "run_subagent" => "Run subagents",
        "remember" => "Remember things",
        "forget_memory" => "Forget memories",
        "connected_apps_execute_tool" => "Use a connected app",
        "routine_create" => "Create routines",
        "routine_pause" => "Pause routines",
        "routine_resume" => "Resume routines",
        "skill_save_recent_work" => "Save skills",
        "skill_attach" => "Attach skills",
        "skill_detach" => "Remove skills",
        "github_publish_pull_request" => "Publish to GitHub",
        _ => "This action",
    }
}

pub fn policy_denied_message(name: &str) -> String {
    match name {
        "workspace_write" => "This Bot is not allowed to write files.".into(),
        "workspace_exec" | "github_run_check" => {
            "This Bot is not allowed to run terminal commands.".into()
        }
        "browser_navigate" => "This Bot is not allowed to open web pages.".into(),
        "browser_click" => "This Bot is not allowed to click in the browser.".into(),
        "browser_type" => "This Bot is not allowed to type in the browser.".into(),
        "browser_screenshot" => "This Bot is not allowed to take screenshots.".into(),
        "browser_download" => "This Bot is not allowed to download files.".into(),
        "bot_delegate" => "This Bot is not allowed to hand work to another Bot.".into(),
        "run_subagent" => "This Bot is not allowed to run subagents.".into(),
        "remember" => "This Bot is not allowed to save memories.".into(),
        "forget_memory" => "This Bot is not allowed to forget memories.".into(),
        "connected_apps_execute_tool" => {
            "This Bot is not allowed to use this connected app.".into()
        }
        "routine_create" => "This Bot is not allowed to create routines.".into(),
        "routine_pause" => "This Bot is not allowed to pause routines.".into(),
        "routine_resume" => "This Bot is not allowed to resume routines.".into(),
        "skill_save_recent_work" => "This Bot is not allowed to save skills.".into(),
        "skill_attach" => "This Bot is not allowed to attach skills.".into(),
        "skill_detach" => "This Bot is not allowed to remove skills.".into(),
        "github_publish_pull_request" => {
            "This Bot is not allowed to publish pull requests to GitHub.".into()
        }
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
    "ask_user",
    "attachment_list",
    "attachment_read",
    "bot_list",
    "bot_delegate",
    "run_subagent",
    "github_list_repositories",
    "github_search_repositories",
    "github_get_repository",
    "github_get_file_contents",
    "github_list_issues",
    "github_get_issue",
    "github_list_pull_requests",
    "github_get_pull_request",
    "github_open_repository",
    "github_run_check",
    "github_review_publish",
    "github_publish_pull_request",
    "connected_apps_search_tools",
    "connected_apps_load_tool",
    "connected_apps_execute_tool",
    "recall_memory",
    "remember",
    "forget_memory",
    "routine_list",
    "routine_create",
    "routine_pause",
    "routine_resume",
    "skill_list",
    "skill_save_recent_work",
    "skill_attach",
    "skill_detach",
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
        assert!(!is_policy_overridable_tool("ask_user"));
        assert!(is_policy_non_overridable_tool("ask_user"));
        assert!(!is_policy_overridable_tool("attachment_list"));
        assert!(!is_policy_overridable_tool("attachment_read"));
        assert!(ALL_AGENT_TOOL_NAMES.contains(&"ask_user"));
        assert!(ALL_AGENT_TOOL_NAMES.contains(&"attachment_list"));
        assert!(ALL_AGENT_TOOL_NAMES.contains(&"attachment_read"));
        assert!(!is_policy_overridable_tool("workspace_read"));
        assert!(!is_policy_overridable_tool("github_list_repositories"));
        assert!(!is_policy_overridable_tool("connected_apps_execute_tool"));
        assert!(!is_policy_overridable_tool("recall_memory"));
        assert!(is_policy_overridable_tool("remember"));
        assert!(is_policy_overridable_tool("forget_memory"));
        assert!(!is_policy_overridable_tool("not_a_tool"));
        assert!(ALL_AGENT_TOOL_NAMES.contains(&"recall_memory"));
        assert!(ALL_AGENT_TOOL_NAMES.contains(&"remember"));
        assert!(ALL_AGENT_TOOL_NAMES.contains(&"forget_memory"));
        assert_eq!(
            policy_action_group("remember"),
            Some(PolicyActionGroup::Memory)
        );
        assert!(!is_memory_tool("run_subagent"));
        assert_eq!(CONNECTED_APPS_TOOL_NAMES.len(), 3);
        assert!(ALL_AGENT_TOOL_NAMES.contains(&"run_subagent"));
        assert!(ALL_AGENT_TOOL_NAMES.contains(&"connected_apps_search_tools"));
        assert!(ALL_AGENT_TOOL_NAMES.contains(&"connected_apps_load_tool"));
        assert!(ALL_AGENT_TOOL_NAMES.contains(&"connected_apps_execute_tool"));
        assert_eq!(
            policy_action_group("connected_apps_execute_tool"),
            Some(PolicyActionGroup::ConnectedApps)
        );
        assert_eq!(
            policy_action_group("run_subagent"),
            Some(PolicyActionGroup::Delegation)
        );
        assert!(is_subagent_tool("run_subagent"));
        assert!(!is_collaboration_tool("run_subagent"));
    }
}
