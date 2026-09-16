//! Slack Events API inbound normalization and v1 filters.

use serde_json::Value;

use crate::channels::types::{ExternalThreadRef, NormalizedInbound};
use crate::channels::ChannelProvider;

pub fn url_verification_challenge(body: &Value) -> Option<String> {
    if body.get("type").and_then(Value::as_str) != Some("url_verification") {
        return None;
    }
    body.get("challenge")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

pub fn normalize_slack_event(envelope: &Value) -> Option<NormalizedInbound> {
    if envelope.get("type").and_then(Value::as_str) != Some("event_callback") {
        return None;
    }
    let event_id = envelope.get("event_id")?.as_str()?.to_string();
    let workspace_id = envelope.get("team_id")?.as_str()?.to_string();
    let event = envelope.get("event")?;
    let event_type = event.get("type")?.as_str()?.to_string();
    match event_type.as_str() {
        "app_mention" => normalize_app_mention(&event_id, &workspace_id, event),
        "message" => normalize_im_message(&event_id, &workspace_id, event),
        _ => None,
    }
}

fn normalize_app_mention(
    event_id: &str,
    workspace_id: &str,
    event: &Value,
) -> Option<NormalizedInbound> {
    let channel_id = event.get("channel")?.as_str()?.to_string();
    let ts = event.get("ts")?.as_str()?.to_string();
    let thread_id = event
        .get("thread_ts")
        .and_then(Value::as_str)
        .unwrap_or(&ts)
        .to_string();
    let sender = event.get("user")?.as_str()?.to_string();
    let raw_text = event.get("text").and_then(Value::as_str).unwrap_or("");
    Some(NormalizedInbound {
        provider: ChannelProvider::Slack,
        event_id: event_id.to_string(),
        event_type: "app_mention".into(),
        workspace_id: workspace_id.to_string(),
        sender_user_id: sender,
        text: strip_slack_mentions(raw_text),
        thread: ExternalThreadRef {
            provider: ChannelProvider::Slack,
            workspace_id: workspace_id.to_string(),
            channel_id,
            thread_id,
            reply_in_thread: true,
        },
        is_bot: event.get("bot_id").is_some() || event.get("bot_profile").is_some(),
        subtype: event
            .get("subtype")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn normalize_im_message(
    event_id: &str,
    workspace_id: &str,
    event: &Value,
) -> Option<NormalizedInbound> {
    let channel_type = event.get("channel_type").and_then(Value::as_str);
    if channel_type != Some("im") {
        return None;
    }
    let channel_id = event.get("channel")?.as_str()?.to_string();
    let sender = event.get("user")?.as_str()?.to_string();
    let raw_text = event.get("text").and_then(Value::as_str).unwrap_or("");
    let explicit_thread = event.get("thread_ts").and_then(Value::as_str);
    let (thread_id, reply_in_thread) = match explicit_thread {
        Some(thread_ts) => (thread_ts.to_string(), true),
        None => (channel_id.clone(), false),
    };
    Some(NormalizedInbound {
        provider: ChannelProvider::Slack,
        event_id: event_id.to_string(),
        event_type: "message.im".into(),
        workspace_id: workspace_id.to_string(),
        sender_user_id: sender,
        text: strip_slack_mentions(raw_text),
        thread: ExternalThreadRef {
            provider: ChannelProvider::Slack,
            workspace_id: workspace_id.to_string(),
            channel_id,
            thread_id,
            reply_in_thread,
        },
        is_bot: event.get("bot_id").is_some()
            || event.get("bot_profile").is_some()
            || event.get("subtype").and_then(Value::as_str) == Some("bot_message"),
        subtype: event
            .get("subtype")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

pub fn strip_slack_mentions(text: &str) -> String {
    let mut out = text.trim().to_string();
    while let Some(start) = out.find("<@") {
        let rest = &out[start..];
        let Some(end) = rest.find('>') else { break };
        out.replace_range(start..start + end + 1, " ");
        out = out.split_whitespace().collect::<Vec<_>>().join(" ");
    }
    out.trim().to_string()
}

pub fn should_ignore(
    inbound: &NormalizedInbound,
    bot_user_id: Option<&str>,
) -> Option<&'static str> {
    if inbound.is_bot {
        return Some("bot_message");
    }
    if let Some(bot_user_id) = bot_user_id {
        if inbound.sender_user_id == bot_user_id {
            return Some("self_message");
        }
    }
    if inbound.event_type == "message.im" {
        if let Some(subtype) = inbound.subtype.as_deref() {
            if !subtype.is_empty() {
                return Some("message_subtype");
            }
        }
    }
    if inbound.event_type == "app_mention" {
        if let Some(subtype) = inbound.subtype.as_deref() {
            if matches!(
                subtype,
                "message_changed" | "message_deleted" | "bot_message"
            ) {
                return Some("message_subtype");
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn app_mention_uses_thread_ts_or_ts() {
        let threaded = json!({
            "type": "event_callback",
            "event_id": "Ev1",
            "team_id": "T1",
            "event": {
                "type": "app_mention",
                "user": "U1",
                "text": "<@UBOT> hello",
                "ts": "1.0",
                "thread_ts": "0.5",
                "channel": "C1"
            }
        });
        let inbound = normalize_slack_event(&threaded).unwrap();
        assert_eq!(inbound.thread.thread_id, "0.5");
        assert!(inbound.thread.reply_in_thread);
        assert_eq!(inbound.text, "hello");

        let root = json!({
            "type": "event_callback",
            "event_id": "Ev2",
            "team_id": "T1",
            "event": {
                "type": "app_mention",
                "user": "U1",
                "text": "hi",
                "ts": "1.0",
                "channel": "C1"
            }
        });
        assert_eq!(
            normalize_slack_event(&root).unwrap().thread.thread_id,
            "1.0"
        );
    }

    #[test]
    fn dm_without_thread_maps_to_channel_id() {
        let dm = json!({
            "type": "event_callback",
            "event_id": "Ev3",
            "team_id": "T1",
            "event": {
                "type": "message",
                "channel_type": "im",
                "channel": "D1",
                "user": "U1",
                "text": "hello",
                "ts": "2.0"
            }
        });
        let inbound = normalize_slack_event(&dm).unwrap();
        assert_eq!(inbound.thread.thread_id, "D1");
        assert!(!inbound.thread.reply_in_thread);
    }
}
