use agent_core::{
    CreateResponseRequest, CreateResponseResult, ModelError, ResponsesModel,
};
use async_trait::async_trait;
use reqwest::Client;

use crate::error::AppError;
use crate::openai::{create_response, ResponsesCreateRequest};

pub struct OpenAiResponsesModel {
    client: Client,
    api_key: String,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl OpenAiResponsesModel {
    pub fn new(api_key: String, cancel: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Self {
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
        if self.cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(ModelError::Cancelled);
        }
        let result = create_response(
            &self.client,
            &self.api_key,
            ResponsesCreateRequest {
                model: request.model,
                instructions: request.instructions,
                input: request.input,
                tools: request.tools,
                tool_choice: request.tool_choice,
                stream: Some(false),
            },
        )
        .await
        .map_err(map_app_error)?;

        Ok(CreateResponseResult {
            output: result.output,
            output_text: result.output_text,
        })
    }
}

fn map_app_error(err: AppError) -> ModelError {
    match err {
        AppError::ModelUnavailable(m) => ModelError::Unavailable(m),
        AppError::RateLimited(m) => ModelError::RateLimited(m),
        AppError::Network(m) => ModelError::Network(m),
        AppError::Provider(m) => ModelError::Provider(m),
        AppError::Cancelled => ModelError::Cancelled,
        other => ModelError::Provider(other.to_string()),
    }
}
