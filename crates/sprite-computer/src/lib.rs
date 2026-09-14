//! Fly Sprites REST adapter implementing [`agent_core::AgentComputer`].

mod client;
mod computer;
mod policy;
mod types;

pub use client::{SpriteClient, SpriteClientConfig};
pub use computer::{SpriteComputer, SpriteComputerConfig};
pub use policy::{default_deny_network_policy, NetworkPolicyConfig};
pub use types::{
    sanitize_sprite_name, sprite_name_for_sandbox, Checkpoint, SpriteError, SpriteInfo,
    DEFAULT_API_BASE, DEFAULT_WORKSPACE_ROOT,
};
