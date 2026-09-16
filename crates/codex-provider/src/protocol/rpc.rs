use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::CodexProviderError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Number(i64),
    Text(String),
}

#[derive(Debug, Clone)]
pub enum IncomingMessage {
    Response {
        id: RequestId,
        result: Value,
    },
    Error {
        id: RequestId,
        error: JsonRpcErrorBody,
    },
    Notification {
        method: String,
        params: Value,
    },
    ServerRequest {
        id: RequestId,
        method: String,
        params: Value,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcErrorBody {
    pub code: i64,
    pub message: String,
    #[serde(default)]
    pub data: Option<Value>,
}

pub fn parse_incoming_line(line: &str) -> Result<Option<IncomingMessage>, CodexProviderError> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value: Value = serde_json::from_str(trimmed)?;
    if value.get("id").is_some() {
        if let Some(error) = value.get("error") {
            let id = parse_request_id(value.get("id"))?;
            let error: JsonRpcErrorBody = serde_json::from_value(error.clone())?;
            return Ok(Some(IncomingMessage::Error { id, error }));
        }
        if value.get("result").is_some() {
            let id = parse_request_id(value.get("id"))?;
            return Ok(Some(IncomingMessage::Response {
                id,
                result: value.get("result").cloned().unwrap_or(Value::Null),
            }));
        }
        if value.get("method").is_some() {
            let id = parse_request_id(value.get("id"))?;
            let method = value
                .get("method")
                .and_then(|m| m.as_str())
                .unwrap_or_default()
                .to_string();
            let params = value.get("params").cloned().unwrap_or(Value::Null);
            return Ok(Some(IncomingMessage::ServerRequest { id, method, params }));
        }
    }

    if let Some(method) = value.get("method").and_then(|m| m.as_str()) {
        let params = value.get("params").cloned().unwrap_or(Value::Null);
        return Ok(Some(IncomingMessage::Notification {
            method: method.to_string(),
            params,
        }));
    }

    Err(CodexProviderError::Protocol(format!(
        "unrecognized app-server message: {trimmed}"
    )))
}

pub fn parse_request_id(id: Option<&Value>) -> Result<RequestId, CodexProviderError> {
    match id {
        Some(Value::Number(n)) => n
            .as_i64()
            .map(RequestId::Number)
            .ok_or_else(|| CodexProviderError::Protocol("invalid numeric id".into())),
        Some(Value::String(s)) => Ok(RequestId::Text(s.clone())),
        other => Err(CodexProviderError::Protocol(format!(
            "missing or invalid request id: {other:?}"
        ))),
    }
}

pub fn request_envelope(id: i64, method: &str, params: Value) -> Value {
    serde_json::json!({
        "id": id,
        "method": method,
        "params": params
    })
}

pub fn notification_envelope(method: &str, params: Option<Value>) -> Value {
    match params {
        Some(p) => serde_json::json!({ "method": method, "params": p }),
        None => serde_json::json!({ "method": method }),
    }
}

pub fn response_envelope(id: &RequestId, result: Value) -> Value {
    match id {
        RequestId::Number(n) => serde_json::json!({ "id": n, "result": result }),
        RequestId::Text(s) => serde_json::json!({ "id": s, "result": result }),
    }
}

pub fn error_envelope(id: &RequestId, code: i64, message: &str) -> Value {
    match id {
        RequestId::Number(n) => serde_json::json!({
            "id": n,
            "error": { "code": code, "message": message }
        }),
        RequestId::Text(s) => serde_json::json!({
            "id": s,
            "error": { "code": code, "message": message }
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_response_and_notification() {
        let response = parse_incoming_line(r#"{"id":1,"result":{"requiresOpenaiAuth":false}}"#)
            .unwrap()
            .unwrap();
        assert!(matches!(response, IncomingMessage::Response { .. }));

        let notification = parse_incoming_line(
            r#"{"method":"account/login/completed","params":{"success":true}}"#,
        )
        .unwrap()
        .unwrap();
        assert!(matches!(notification, IncomingMessage::Notification { .. }));
    }
}
