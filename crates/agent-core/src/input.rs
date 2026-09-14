use serde_json::{json, Value};

use crate::events::RuntimeError;
use crate::model::function_call_output_item;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageStatus {
    Pending,
    Streaming,
    Complete,
    Error,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone)]
pub struct ConversationMessage {
    pub role: MessageRole,
    pub kind: String,
    pub body: String,
    pub status: MessageStatus,
}

pub fn build_responses_input(messages: &[ConversationMessage]) -> Result<Vec<Value>, RuntimeError> {
    let mut input = Vec::new();
    for message in messages {
        if matches!(
            message.status,
            MessageStatus::Error | MessageStatus::Cancelled | MessageStatus::Interrupted
        ) {
            continue;
        }
        match message.kind.as_str() {
            "text" => match message.role {
                MessageRole::User => input.push(json!({
                    "role": "user",
                    "content": message.body
                })),
                MessageRole::Assistant => {
                    if !message.body.trim().is_empty() {
                        input.push(json!({
                            "role": "assistant",
                            "content": message.body
                        }));
                    }
                }
                MessageRole::System => {}
            },
            "tool_call" => {
                let parsed: Value = serde_json::from_str(&message.body).map_err(|e| {
                    RuntimeError::Validation(format!("invalid tool_call body: {e}"))
                })?;
                let name = parsed.get("tool").and_then(|v| v.as_str()).unwrap_or("");
                let call_id = parsed.get("callId").and_then(|v| v.as_str()).unwrap_or("");
                let arguments = parsed.get("arguments").cloned().unwrap_or(json!({}));
                input.push(json!({
                    "type": "function_call",
                    "name": name,
                    "call_id": call_id,
                    "arguments": serde_json::to_string(&arguments).unwrap_or_else(|_| "{}".into())
                }));
            }
            "tool_result" => {
                let parsed: Value = serde_json::from_str(&message.body).map_err(|e| {
                    RuntimeError::Validation(format!("invalid tool_result body: {e}"))
                })?;
                let call_id = parsed.get("callId").and_then(|v| v.as_str()).unwrap_or("");
                let output = parsed
                    .get("output")
                    .and_then(|v| v.as_str())
                    .unwrap_or("{}");
                input.push(function_call_output_item(call_id, output));
            }
            _ => {}
        }
    }
    Ok(input)
}
