use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("unavailable: {0}")]
    Unavailable(String),
    #[error("rate limited: {0}")]
    RateLimited(String),
    #[error("network: {0}")]
    Network(String),
    #[error("provider: {0}")]
    Provider(String),
    #[error("cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateResponseRequest {
    pub model: String,
    pub instructions: Option<String>,
    pub input: Value,
    pub tools: Value,
    pub tool_choice: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateResponseResult {
    pub output: Vec<Value>,
    #[serde(default)]
    pub output_text: Option<String>,
}

#[async_trait]
pub trait ResponsesModel: Send + Sync {
    async fn create_response(
        &self,
        request: CreateResponseRequest,
    ) -> Result<CreateResponseResult, ModelError>;
}

pub fn model_supports_responses_tools(model: &str) -> bool {
    let id = model.to_lowercase();
    if id.contains("embedding")
        || id.contains("tts")
        || id.contains("whisper")
        || id.contains("dall-e")
        || id.contains("image")
        || id.contains("realtime")
    {
        return false;
    }
    id.starts_with("gpt-")
        || id.starts_with("o1")
        || id.starts_with("o3")
        || id.starts_with("o4")
        || id.starts_with("chatgpt-")
}

pub fn extract_function_calls(output: &[Value]) -> Vec<(String, String, String)> {
    let mut calls = Vec::new();
    for item in output {
        if item.get("type").and_then(|v| v.as_str()) != Some("function_call") {
            continue;
        }
        let name = item
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let call_id = item
            .get("call_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let arguments = item
            .get("arguments")
            .and_then(|v| v.as_str())
            .unwrap_or("{}")
            .to_string();
        if !name.is_empty() && !call_id.is_empty() {
            calls.push((name, call_id, arguments));
        }
    }
    calls
}

pub fn extract_assistant_text(output: &[Value], output_text: Option<&str>) -> String {
    if let Some(text) = output_text {
        if !text.trim().is_empty() {
            return text.to_string();
        }
    }
    for item in output {
        if item.get("type").and_then(|v| v.as_str()) != Some("message") {
            continue;
        }
        if let Some(content) = item.get("content").and_then(|v| v.as_array()) {
            let mut parts = Vec::new();
            for block in content {
                if block.get("type").and_then(|v| v.as_str()) == Some("output_text") {
                    if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                        parts.push(text);
                    }
                }
            }
            if !parts.is_empty() {
                return parts.join("");
            }
        }
    }
    String::new()
}

pub fn function_call_output_item(call_id: &str, output: &str) -> Value {
    json!({
        "type": "function_call_output",
        "call_id": call_id,
        "output": output
    })
}
