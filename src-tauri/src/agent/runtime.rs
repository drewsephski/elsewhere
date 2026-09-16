use crate::error::AppError;
use crate::models::Message;
use agent_core::{
    build_responses_input, run_agent_loop, AgentLoopContext, AgentLoopDeps, ConversationMessage,
    MessageRole as CoreRole, MessageStatus as CoreStatus,
};
pub fn build_responses_input_from_messages(
    messages: &[Message],
) -> Result<Vec<serde_json::Value>, AppError> {
    let mapped: Vec<ConversationMessage> = messages
        .iter()
        .map(|m| ConversationMessage {
            role: match m.role {
                crate::models::MessageRole::User => CoreRole::User,
                crate::models::MessageRole::Assistant => CoreRole::Assistant,
                crate::models::MessageRole::System => CoreRole::System,
            },
            kind: m.kind.clone(),
            body: m.body.clone(),
            status: match m.status {
                crate::models::MessageStatus::Pending => CoreStatus::Pending,
                crate::models::MessageStatus::Streaming => CoreStatus::Streaming,
                crate::models::MessageStatus::Complete => CoreStatus::Complete,
                crate::models::MessageStatus::Error => CoreStatus::Error,
                crate::models::MessageStatus::Cancelled => CoreStatus::Cancelled,
                crate::models::MessageStatus::Interrupted => CoreStatus::Interrupted,
            },
        })
        .collect();
    build_responses_input(&mapped).map_err(|e| AppError::Validation(e.to_string()))
}

pub async fn run_agent_chat(
    deps: AgentLoopDeps,
    ctx: AgentLoopContext,
    input: Vec<serde_json::Value>,
) -> Result<(), AppError> {
    run_agent_loop(ctx, deps, input)
        .await
        .map_err(|e| AppError::Provider(e.to_string()))
}
