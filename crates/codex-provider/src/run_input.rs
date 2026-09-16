use serde_json::{json, Value};
use std::path::Path;

use agent_core::{AttachmentKind, RuntimeError, SharedRunDeps};

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
                    Some("input_image") | Some("image") | Some("localImage") => {}
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

pub async fn build_codex_turn_input(
    shared: &SharedRunDeps,
    cwd: &Path,
    fallback_text: &str,
) -> Result<Vec<Value>, RuntimeError> {
    let text = if !shared.user_input.text.trim().is_empty() {
        shared.user_input.text.clone()
    } else {
        fallback_text.to_string()
    };
    let mut input = Vec::new();
    if !text.trim().is_empty() {
        input.push(json!({"type": "text", "text": text}));
    }
    let inbox = cwd.join("elsewhere-inputs");
    for descriptor in &shared.user_input.attachments {
        if descriptor.kind != AttachmentKind::Image {
            continue;
        }
        let backend = shared.attachments.as_ref().ok_or_else(|| {
            RuntimeError::Validation("image attachments require an attachment store".into())
        })?;
        let (loaded, bytes) = backend
            .load_image_bytes(&descriptor.id)
            .await
            .map_err(|e| RuntimeError::Validation(format!("attachment image: {e:?}")))?;
        std::fs::create_dir_all(&inbox)
            .map_err(|e| RuntimeError::Validation(format!("attachment temp dir: {e}")))?;
        let ext = loaded.kind.extension(&loaded.mime_type);
        let id8: String = loaded
            .id
            .chars()
            .filter(|c| c.is_ascii_hexdigit())
            .take(8)
            .collect();
        let path = inbox.join(format!("{id8}.{ext}"));
        std::fs::write(&path, bytes)
            .map_err(|e| RuntimeError::Validation(format!("write local image: {e}")))?;
        input.push(json!({
            "type": "localImage",
            "path": format!("elsewhere-inputs/{id8}.{ext}")
        }));
    }
    if input.is_empty() {
        return Err(RuntimeError::Validation(
            "Codex run requires text or at least one image attachment".into(),
        ));
    }
    Ok(input)
}
