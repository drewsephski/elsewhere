//! Codex app-server client, MCP bridge integration, and subscription run engine scaffold.

mod client;
mod compat;
mod error;
mod login;
mod process;
mod protocol;
mod run_engine;
mod run_input;
mod run_persistence;

mod probe;

mod testing;

pub use testing::{spawn_fake_app_server, spawn_fake_app_server_with_mode, FakeServerMode};

pub use client::CodexAppServerClient;
pub use compat::{
    generate_schema_to_dir, verify_generated_schema_dir, REQUIRED_PROTOCOL_METHODS,
};
pub use error::CodexProviderError;
pub use process::{codex_version, which_codex_executable, CodexProcessLaunch, ManagedCodexProcess};
pub use protocol::{
    build_elsewhere_thread_start_params, parse_account_response, CodexAccountKind,
    CodexAccountState, CodexLoginHandle, CodexRateLimitsSnapshot, ElsewhereThreadConfig,
    MCP_SERVER_NAME,
};
pub use protocol::thread::assert_host_tools_disabled;
pub use probe::{
    parse_login_status, prefers_chatgpt_subscription, probe_codex, CodexAuthMethod,
    CodexInstallProbe, CodexProbeError,
};
pub use run_engine::{CodexRunEngine, CodexRunEngineConfig};
