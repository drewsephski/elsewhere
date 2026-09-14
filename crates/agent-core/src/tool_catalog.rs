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
