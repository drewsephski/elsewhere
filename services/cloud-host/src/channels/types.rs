//! Provider-neutral messaging concepts. Slack is one implementation.

use async_trait::async_trait;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelProvider {
    Slack,
}

impl ChannelProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Slack => super::PROVIDER_SLACK,
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            super::PROVIDER_SLACK => Some(Self::Slack),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryKind {
    AssistantReply,
    OwnerAttention,
}

impl DeliveryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AssistantReply => "assistant_reply",
            Self::OwnerAttention => "owner_attention",
        }
    }
}

/// Identity of an external conversation/thread, stable across provider retries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalThreadRef {
    pub provider: ChannelProvider,
    pub workspace_id: String,
    pub channel_id: String,
    /// Root thread timestamp for Slack mentions; the IM channel id for unthreaded DMs.
    pub thread_id: String,
    /// When true, outbound replies should include `thread_ts`.
    pub reply_in_thread: bool,
}

#[derive(Debug, Clone)]
pub struct NormalizedInbound {
    pub provider: ChannelProvider,
    pub event_id: String,
    pub event_type: String,
    pub workspace_id: String,
    pub sender_user_id: String,
    pub text: String,
    pub thread: ExternalThreadRef,
    pub is_bot: bool,
    pub subtype: Option<String>,
}

impl NormalizedInbound {
    pub fn to_payload(&self) -> serde_json::Value {
        serde_json::json!({
            "provider": self.provider.as_str(),
            "eventId": self.event_id,
            "eventType": self.event_type,
            "workspaceId": self.workspace_id,
            "senderUserId": self.sender_user_id,
            "text": self.text,
            "channelId": self.thread.channel_id,
            "threadId": self.thread.thread_id,
            "replyInThread": self.thread.reply_in_thread,
            "isBot": self.is_bot,
            "subtype": self.subtype,
        })
    }

    pub fn from_payload(value: &serde_json::Value) -> Option<Self> {
        let provider = ChannelProvider::parse(value.get("provider")?.as_str()?)?;
        let workspace_id = value.get("workspaceId")?.as_str()?.to_string();
        let channel_id = value.get("channelId")?.as_str()?.to_string();
        let thread_id = value.get("threadId")?.as_str()?.to_string();
        Some(Self {
            provider,
            event_id: value.get("eventId")?.as_str()?.to_string(),
            event_type: value.get("eventType")?.as_str()?.to_string(),
            workspace_id: workspace_id.clone(),
            sender_user_id: value.get("senderUserId")?.as_str()?.to_string(),
            text: value.get("text")?.as_str().unwrap_or("").to_string(),
            thread: ExternalThreadRef {
                provider,
                workspace_id,
                channel_id,
                thread_id,
                reply_in_thread: value
                    .get("replyInThread")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
            },
            is_bot: value
                .get("isBot")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            subtype: value
                .get("subtype")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
        })
    }
}

#[derive(Debug, Clone)]
pub struct OutboundDelivery {
    pub channel_id: String,
    pub thread_ts: Option<String>,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct DeliveryReceipt {
    pub provider_message_id: String,
}

/// Extensible provider surface: inbound normalization + outbound delivery.
/// Connection identity lives in `channel_connections`; this trait does not
/// own persistence.
#[async_trait]
pub trait MessagingProvider: Send + Sync {
    fn provider(&self) -> ChannelProvider;

    fn normalize_inbound(&self, envelope: &serde_json::Value) -> Option<NormalizedInbound>;

    async fn deliver(
        &self,
        token: &str,
        message: &OutboundDelivery,
    ) -> Result<DeliveryReceipt, ProviderDeliveryError>;
}

#[derive(Debug, Clone)]
pub enum ProviderDeliveryError {
    Retryable {
        message: String,
        retry_after_secs: Option<u64>,
    },
    Permanent {
        message: String,
    },
}

impl std::fmt::Display for ProviderDeliveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Retryable { message, .. } | Self::Permanent { message } => f.write_str(message),
        }
    }
}
