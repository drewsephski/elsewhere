//! Provider-neutral messaging channels. Slack is the first provider.
//!
//! Connected apps are capabilities the Bot *calls*. Channels are surfaces
//! through which a human *talks to* the Bot. This module is not part of
//! `AgentComputer`.

pub mod admission;
pub mod db;
pub mod delivery;
pub mod oauth;
pub mod slack;
pub mod types;

pub use types::{
    ChannelProvider, DeliveryKind, ExternalThreadRef, MessagingProvider, NormalizedInbound,
    OutboundDelivery,
};

pub const PROVIDER_SLACK: &str = "slack";
pub const ORIGIN_WEB: &str = "web";
pub const ORIGIN_CHANNEL: &str = "channel";
pub const MAX_INBOUND_TEXT_BYTES: usize = 8_000;
pub const MAX_EVENTS_PER_CONNECTION_PER_MINUTE: i64 = 30;
pub const MAX_EVENT_BODY_BYTES: usize = 256 * 1024;
pub const MAX_DELIVERY_BODY_CHARS: usize = 3_500;
pub const MAX_DELIVERY_CHUNKS: usize = 3;
pub const SLACK_BOT_SCOPES: &str = "chat:write,app_mentions:read,im:history";
pub const SLACK_TIMESTAMP_SKEW_SECS: i64 = 60 * 5;

pub fn origin_label(origin_kind: &str, origin_provider: Option<&str>) -> Option<String> {
    if origin_kind != ORIGIN_CHANNEL {
        return None;
    }
    Some(match origin_provider {
        Some(PROVIDER_SLACK) => "Slack".into(),
        Some(other) if !other.is_empty() => other.to_string(),
        _ => "a connected channel".into(),
    })
}

pub fn work_url(web_origin: Option<&str>, run_id: &str) -> Option<String> {
    let origin = web_origin?.trim().trim_end_matches('/');
    if origin.is_empty() {
        return None;
    }
    Some(format!("{origin}/app/work/{run_id}"))
}
