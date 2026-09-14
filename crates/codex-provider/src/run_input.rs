use serde_json::Value;

use agent_core::RuntimeError;

pub fn user_text_from_run_input(input: &[Value]) -> Result<String, RuntimeError> {
    let mut segments = Vec::new();
    for item in input {
        if let Some(text) = extract_user_text(item)? {
            segments.push(text);
        }
    }
    if segments.is_empty() {
        return Err(RuntimeError::Validation(
            "Codex run requires at least one textual user message".into(),
        ));
    }
    Ok(segments.join("\n\n"))
}

fn extract_user_text(item: &Value) -> Result<Option<String>, RuntimeError> {
    if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
        if item.get("type").and_then(|v| v.as_str()) == Some("text") {
            return Ok(Some(text.to_string()));
        }
    }
    let role = item.get("role").and_then(|v| v.as_str());
    if role != Some("user") {
        if role.is_some() {
            return Err(RuntimeError::Validation(
                "unsupported input role for Codex subscription run (only user text supported)"
                    .into(),
            ));
        }
        return Ok(None);
    }
    match item.get("content") {
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(Value::Array(parts)) => {
            let mut text = String::new();
            for part in parts {
                match part.get("type").and_then(|v| v.as_str()) {
                    Some("text") | Some("input_text") | Some("output_text") => {
                        if let Some(chunk) = part.get("text").and_then(|v| v.as_str()) {
                            text.push_str(chunk);
                        }
                    }
                    other => {
                        return Err(RuntimeError::Validation(format!(
                            "unsupported user content part type: {other:?}"
                        )));
                    }
                }
            }
            if text.is_empty() {
                Ok(None)
            } else {
                Ok(Some(text))
            }
        }
        Some(_) => Err(RuntimeError::Validation(
            "unsupported user content shape for Codex subscription run".into(),
        )),
        None => Ok(None),
    }
}
