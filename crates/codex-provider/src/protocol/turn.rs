use serde_json::{json, Value};

use crate::error::CodexProviderError;

pub fn build_turn_start_params(thread_id: &str, user_text: &str) -> Value {
    json!({
        "threadId": thread_id,
        "input": [{
            "type": "text",
            "text": user_text
        }]
    })
}

pub fn build_turn_interrupt_params(thread_id: &str, turn_id: &str) -> Value {
    json!({
        "threadId": thread_id,
        "turnId": turn_id
    })
}

pub fn parse_turn_start_response(value: Value) -> Result<String, CodexProviderError> {
    value
        .get("turn")
        .and_then(|t| t.get("id"))
        .or_else(|| value.get("turnId"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| CodexProviderError::Protocol("turn/start missing turn id".into()))
}

pub fn parse_turn_completed(
    params: &Value,
) -> Result<(String, String, String), CodexProviderError> {
    let thread_id = params
        .get("threadId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CodexProviderError::Protocol("turn/completed missing threadId".into()))?
        .to_string();
    let turn = params
        .get("turn")
        .ok_or_else(|| CodexProviderError::Protocol("turn/completed missing turn".into()))?;
    let turn_id = turn
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CodexProviderError::Protocol("turn/completed missing turn.id".into()))?
        .to_string();
    let status = turn
        .get("status")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CodexProviderError::Protocol("turn/completed missing turn.status".into()))?
        .to_string();
    Ok((thread_id, turn_id, status))
}

pub fn turn_error_message(params: &Value) -> Option<String> {
    params
        .get("turn")
        .and_then(|t| t.get("error"))
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .map(str::to_string)
}

pub fn notification_thread_turn(params: &Value) -> (Option<String>, Option<String>) {
    let thread_id = params.get("threadId").and_then(|v| v.as_str());
    let turn_id = params
        .get("turnId")
        .or_else(|| params.get("turn").and_then(|t| t.get("id")))
        .and_then(|v| v.as_str());
    (thread_id.map(str::to_string), turn_id.map(str::to_string))
}

pub fn item_from_notification(params: &Value) -> Option<&Value> {
    params.get("item")
}
