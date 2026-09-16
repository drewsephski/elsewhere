use agent_core::{CreateResponseRequest, CreateResponseResult, ModelError, ResponsesModel};
use async_trait::async_trait;
use openai_responses::OpenAiResponsesModel as InnerModel;

pub struct OpenAiResponsesModel {
    inner: InnerModel,
}

impl OpenAiResponsesModel {
    pub fn new(api_key: String, cancel: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Self {
        Self {
            inner: InnerModel::new(api_key, cancel),
        }
    }
}

#[async_trait]
impl ResponsesModel for OpenAiResponsesModel {
    async fn create_response(
        &self,
        request: CreateResponseRequest,
    ) -> Result<CreateResponseResult, ModelError> {
        self.inner.create_response(request).await
    }
}
