use serde_json::{json, Value};

use crate::connectors::installs::StoredSecret;

pub fn redact_value(value: &Value, secrets: &[String]) -> Value {
    match value {
        Value::String(s) => Value::String(redact_text(s, secrets)),
        Value::Array(items) => {
            Value::Array(items.iter().map(|v| redact_value(v, secrets)).collect())
        }
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                if is_sensitive_key(k) {
                    out.insert(k.clone(), json!("[redacted]"));
                } else {
                    out.insert(k.clone(), redact_value(v, secrets));
                }
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

pub fn redact_text(input: &str, secrets: &[String]) -> String {
    let mut out = crate::redact::redact_secrets(input);
    for secret in secrets {
        if !secret.is_empty() {
            out = out.replace(secret, "[redacted]");
        }
    }
    out = redact_bearer_forms(&out);
    out
}

fn redact_bearer_forms(input: &str) -> String {
    let mut out = input.to_string();
    for marker in ["Bearer ", "bearer "] {
        while let Some(idx) = out.find(marker) {
            let rest = &out[idx + marker.len()..];
            let end = rest
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ',')
                .unwrap_or(rest.len());
            out.replace_range(idx..idx + marker.len() + end, "[redacted]");
        }
    }
    out
}

fn is_sensitive_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower.contains("authorization")
        || lower.contains("cookie")
        || lower.contains("token")
        || lower.contains("secret")
        || lower.contains("password")
        || lower.contains("apikey")
        || lower.contains("api_key")
}

pub fn secrets_from_stored(secret: Option<&StoredSecret>) -> Vec<String> {
    secret.map(|s| s.secret_values()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_bearer_and_secret_values() {
        let secret = "super-secret-token-value";
        let redacted = redact_text(
            &format!("failed Bearer {secret} extra"),
            &[secret.to_string()],
        );
        assert!(!redacted.contains(secret));
        assert!(redacted.contains("[redacted]"));
    }
}
