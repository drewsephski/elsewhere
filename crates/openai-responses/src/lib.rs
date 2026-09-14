use agent_core::{
    CreateResponseRequest, CreateResponseResult, ModelError, ResponsesModel,
};
use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const OPENAI_BASE: &str = "https://api.openai.com/v1";

fn openai_base_url() -> String {
    std::env::var("OPENAI_API_BASE").unwrap_or_else(|_| OPENAI_BASE.to_string())
}

#[derive(Debug, Deserialize)]
struct ResponsesApiResult {
    output: Vec<serde_json::Value>,
    #[serde(default)]
    output_text: Option<String>,
}

pub fn map_http_status_to_model_error(status: StatusCode, body: &str) -> ModelError {
    if status == StatusCode::UNAUTHORIZED {
        return ModelError::Provider("invalid OpenAI API key".into());
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        return ModelError::RateLimited(sanitize_provider_body(body));
    }
    if status == StatusCode::NOT_FOUND {
        return ModelError::Unavailable(sanitize_provider_body(body));
    }
    ModelError::Provider(format!("HTTP {}: {}", status, sanitize_provider_body(body)))
}

fn sanitize_provider_body(body: &str) -> String {
    let mut out = body.to_string();
    for key in ["OPENAI_API_KEY", "SPRITE_TOKEN", "ELSEWHERE_CLOUD_API_TOKEN"] {
        if out.contains(key) {
            out = out.replace(key, "[redacted]");
        }
    }
    out
}

pub async fn create_response(
    client: &Client,
    api_key: &str,
    request: CreateResponseRequest,
) -> Result<CreateResponseResult, ModelError> {
    let url = format!("{}/responses", openai_base_url().trim_end_matches('/'));
    let payload = serde_json::json!({
        "model": request.model,
        "instructions": request.instructions,
        "input": request.input,
        "tools": request.tools,
        "tool_choice": request.tool_choice,
        "stream": false
    });

    let response = client
        .post(url)
        .bearer_auth(api_key)
        .json(&payload)
        .send()
        .await
        .map_err(|e| ModelError::Network(e.to_string()))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| ModelError::Network(e.to_string()))?;

    if !status.is_success() {
        return Err(map_http_status_to_model_error(status, &body));
    }

    let parsed: ResponsesApiResult = serde_json::from_str(&body)
        .map_err(|e| ModelError::Provider(format!("malformed responses payload: {e}")))?;

    Ok(CreateResponseResult {
        output: parsed.output,
        output_text: parsed.output_text,
    })
}

pub struct OpenAiResponsesModel {
    client: Client,
    api_key: String,
    cancel: Arc<AtomicBool>,
}

impl OpenAiResponsesModel {
    pub fn new(api_key: String, cancel: Arc<AtomicBool>) -> Self {
        Self {
            client: Client::new(),
            api_key,
            cancel,
        }
    }
}

#[async_trait]
impl ResponsesModel for OpenAiResponsesModel {
    async fn create_response(
        &self,
        request: CreateResponseRequest,
    ) -> Result<CreateResponseResult, ModelError> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(ModelError::Cancelled);
        }
        create_response(&self.client, &self.api_key, request).await
    }
}
