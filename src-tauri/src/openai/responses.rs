use crate::error::AppError;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const OPENAI_BASE: &str = "https://api.openai.com/v1";

#[derive(Debug, Clone, Serialize)]
pub struct ResponsesCreateRequest {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    pub input: Value,
    pub tools: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ResponsesCreateResult {
    pub id: String,
    pub output: Vec<Value>,
    #[serde(default)]
    pub output_text: Option<String>,
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

pub async fn create_response(
    client: &Client,
    api_key: &str,
    request: ResponsesCreateRequest,
) -> Result<ResponsesCreateResult, AppError> {
    let url = format!("{OPENAI_BASE}/responses");
    let response = client
        .post(url)
        .bearer_auth(api_key)
        .json(&request)
        .send()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;

    if !status.is_success() {
        return Err(AppError::from_openai_status(status, &body));
    }

    serde_json::from_str(&body)
        .map_err(|e| AppError::Provider(format!("malformed responses payload: {e}")))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_function_calls_from_output() {
        let output = vec![json!({
            "type": "function_call",
            "name": "workspace_read",
            "call_id": "call_1",
            "arguments": "{\"path\":\"/workspace/a.txt\"}"
        })];
        let calls = extract_function_calls(&output);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "workspace_read");
    }

    #[test]
    fn model_supports_tools_for_gpt_family() {
        assert!(model_supports_responses_tools("gpt-4o-mini"));
        assert!(!model_supports_responses_tools("text-embedding-3-small"));
    }
}
