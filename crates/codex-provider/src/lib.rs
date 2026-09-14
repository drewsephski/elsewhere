//! Interim Codex installation probe for Elsewhere Phase 3B.2.
//!
//! Long term, auth and account state should come from the Codex app-server API,
//! not CLI stdout. See `docs/PHASE_3B2_CODEX_PROVIDER.md`.

mod probe;

pub use probe::{
    parse_login_status, prefers_chatgpt_subscription, probe_codex, CodexAuthMethod,
    CodexInstallProbe, CodexProbeError,
};
