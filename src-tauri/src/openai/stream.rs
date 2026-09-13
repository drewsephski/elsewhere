use crate::error::AppError;
use bytes::Bytes;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const OPENAI_BASE: &str = "https://api.openai.com/v1";

#[derive(Debug, Clone, Serialize)]
pub struct ChatMessageInput {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessageInput>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamDelta {
    content: Option<String>,
}

pub async fn stream_chat_completion<F>(
    api_key: &str,
    model: &str,
    messages: Vec<ChatMessageInput>,
    cancelled: Arc<AtomicBool>,
    mut on_delta: F,
) -> Result<String, AppError>
where
    F: FnMut(&str),
{
    let http = reqwest::Client::new();
    let url = format!("{}/chat/completions", OPENAI_BASE);
    let body = ChatCompletionRequest {
        model: model.to_string(),
        messages,
        stream: true,
    };

    let response = http
        .post(&url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        let text = response
            .text()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?;
        return Err(AppError::from_openai_status(status, &text));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut full = String::new();

    while let Some(chunk) = stream.next().await {
        if cancelled.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled);
        }
        let bytes: Bytes = chunk.map_err(|e| AppError::Network(e.to_string()))?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));

        while let Some(pos) = buffer.find("\n\n") {
            let line_block = buffer[..pos].to_string();
            buffer = buffer[pos + 2..].to_string();

            for line in line_block.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with(':') {
                    continue;
                }
                if !line.starts_with("data: ") {
                    continue;
                }
                let data = line.strip_prefix("data: ").unwrap_or("");
                if data == "[DONE]" {
                    continue;
                }
                let parsed: StreamChunk = serde_json::from_str(data).map_err(|e| {
                    AppError::Provider(format!("malformed stream chunk: {}", e))
                })?;
                for choice in parsed.choices {
                    if let Some(content) = choice.delta.content {
                        if !content.is_empty() {
                            full.push_str(&content);
                            on_delta(&content);
                        }
                    }
                }
            }
        }
    }

    if cancelled.load(Ordering::Relaxed) {
        return Err(AppError::Cancelled);
    }

    Ok(full)
}

/// Used by unit tests to verify chunk assembly semantics.
pub fn assemble_stream_chunks(chunks: &[&str]) -> String {
    let mut full = String::new();
    for c in chunks {
        full.push_str(c);
    }
    full
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_assemble() {
        assert_eq!(
            assemble_stream_chunks(&["Hello", ", ", "world"]),
            "Hello, world"
        );
    }

    #[test]
    fn provider_error_from_status() {
        let err = AppError::from_openai_status(reqwest::StatusCode::UNAUTHORIZED, "bad");
        assert!(matches!(err, AppError::InvalidApiKey));
    }
}
