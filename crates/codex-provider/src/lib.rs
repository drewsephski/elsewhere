//! Codex app-server client, MCP bridge integration, and subscription run engine scaffold.

mod assistant_accumulator;
mod client;
mod compat;
mod error;
mod login;
mod mcp_turn_probe;
mod process;
mod protocol;
mod run_engine;
mod run_input;
mod run_persistence;

mod probe;
mod subscription_availability;

mod testing;

pub use testing::{spawn_fake_app_server, spawn_fake_app_server_with_mode, FakeServerMode};

pub use client::CodexAppServerClient;
pub use compat::{
    ensure_codex_mcp_tool_exposure_supported, generate_schema_to_dir, verify_generated_schema_dir,
    REQUIRED_PROTOCOL_METHODS,
};
pub use error::CodexProviderError;
pub use login::CodexDeviceLoginHandle;
pub use mcp_turn_probe::{
    run_mcp_turn_probe, McpTurnProbeResult, DIRECT_TOOL_PROBE_EXPECTED_CONTENT,
    DIRECT_TOOL_PROBE_PROMPT,
};
pub use probe::{
    parse_login_status, prefers_chatgpt_subscription, probe_codex, CodexAuthMethod,
    CodexInstallProbe, CodexProbeError,
};
pub use process::{codex_version, which_codex_executable, CodexProcessLaunch, ManagedCodexProcess};
pub use protocol::thread::{
    assert_elsewhere_mcp_direct_exposure, assert_host_tools_disabled, ELSEWHERE_ENABLED_MCP_TOOLS,
    ELSEWHERE_OMIT_MCP_TOOL_EXPOSURES,
};
pub use protocol::{
    build_elsewhere_thread_start_params, parse_account_response, CodexAccountKind,
    CodexAccountState, CodexLoginHandle, CodexRateLimitsSnapshot, ElsewhereThreadConfig,
    MCP_SERVER_NAME,
};
pub use run_engine::{CodexRunEngine, CodexRunEngineConfig};
#[cfg(any(test, feature = "test-utils"))]
pub use subscription_availability::probe_codex_subscription_availability_on_client;
pub use subscription_availability::{
    probe_codex_subscription_availability, probe_codex_subscription_availability_with_profile,
    CodexSubscriptionAvailability,
};
