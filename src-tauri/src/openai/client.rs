use crate::error::AppError;
use crate::models::{ModelCapabilities, ModelDescriptor};
use serde::Deserialize;

const OPENAI_BASE: &str = "https://api.openai.com/v1";

pub struct OpenAiClient {
    http: reqwest::Client,
    api_key: String,
}

impl OpenAiClient {
    pub fn new(api_key: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key,
        }
    }

    pub async fn list_models(&self) -> Result<Vec<ModelDescriptor>, AppError> {
        let url = format!("{}/models", OPENAI_BASE);
        let response = self
            .http
            .get(&url)
            .bearer_auth(&self.api_key)
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

        let parsed: ModelsResponse = serde_json::from_str(&body)
            .map_err(|e| AppError::Provider(format!("malformed models response: {}", e)))?;

        let mut models: Vec<ModelDescriptor> = parsed
            .data
            .into_iter()
            .filter(|m| is_chat_model(&m.id))
            .map(|m| {
                let capabilities = model_capabilities(&m.id);
                ModelDescriptor {
                    id: m.id.clone(),
                    display_name: m.id.clone(),
                    provider: "openai".to_string(),
                    capabilities,
                }
            })
            .collect();

        models.sort_by(|a, b| a.id.cmp(&b.id));
        models.dedup_by(|a, b| a.id == b.id);

        if models.is_empty() {
            return Err(AppError::Provider(
                "no chat-capable models returned by OpenAI".into(),
            ));
        }

        Ok(models)
    }
}

pub fn is_chat_model(id: &str) -> bool {
    let id = id.to_lowercase();
    if id.contains("embedding")
        || id.contains("tts")
        || id.contains("whisper")
        || id.contains("dall-e")
        || id.contains("moderation")
        || id.contains("realtime")
        || id.contains("audio")
        || id.contains("transcribe")
        || id.contains("search")
        || id.contains("gpt-image")
    {
        return false;
    }
    id.starts_with("gpt-")
        || id.starts_with("o1")
        || id.starts_with("o3")
        || id.starts_with("o4")
        || id.starts_with("chatgpt-")
}

pub fn model_capabilities(id: &str) -> ModelCapabilities {
    let id_lower = id.to_lowercase();
    let text_output = is_chat_model(id);
    let streaming = text_output && !id_lower.contains("image");
    ModelCapabilities {
        text_output,
        streaming,
        tools: false,
        vision: id_lower.contains("vision") || id_lower.contains("4o"),
    }
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<OpenAiModel>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModel {
    id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excludes_gpt_image_models() {
        assert!(!is_chat_model("gpt-image-2.5-sunburst"));
        assert!(!is_chat_model("gpt-image-2.5-flare"));
        assert!(is_chat_model("gpt-5.6-terra"));
    }
}
