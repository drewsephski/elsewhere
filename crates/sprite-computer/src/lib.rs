//! Fly Sprites REST adapter implementing [`agent_core::AgentComputer`].

mod browser;
mod browser_profile_sync;
mod client;
mod computer;
mod policy;
mod types;

pub use browser::{ensure_browser_guest, with_temporary_egress, BROWSER_ROOT};
pub use browser_profile_sync::{clear_guest_profile, hydrate_from_host, persist_to_host};
pub use client::{SpriteClient, SpriteClientConfig};
pub use computer::{SpriteComputer, SpriteComputerConfig};
pub use policy::{
    browser_workload_network_policy, default_deny_network_policy, network_policy_matches,
    NetworkPolicyConfig,
};
pub use types::{
    sanitize_sprite_name, sprite_name_for_sandbox, Checkpoint, SpriteError, SpriteInfo,
    DEFAULT_API_BASE, DEFAULT_WORKSPACE_ROOT,
};
