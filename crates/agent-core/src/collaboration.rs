//! Provider-neutral bot collaboration (not part of AgentComputer).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub const MAX_DELEGATION_DEPTH: i32 = 4;
pub const MAX_CHILD_DELEGATIONS_PER_ROOT: i64 = 8;
pub const MAX_DELEGATION_INSTRUCTION_CHARS: usize = 10_000;
pub const MAX_DELEGATION_CONTEXT_CHARS: usize = 4_000;

#[derive(Debug, Clone)]
pub struct CollaborationContext {
    pub owner_id: String,
    pub source_bot_id: String,
    pub source_run_id: String,
    pub source_conversation_id: String,
    pub source_request_id: String,
    pub tool_invocation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BotTeammateSummary {
    pub id: String,
    pub name: String,
    pub role_summary: String,
    pub available: bool,
    pub is_self: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelegationEnqueueResult {
    pub delegation_id: String,
    pub target_bot_id: String,
    pub target_bot_name: String,
    pub target_run_id: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollaborationError {
    NotFound,
    Validation(String),
    LimitExceeded(String),
    Conflict(String),
    Internal(String),
}

impl CollaborationError {
    pub fn code(&self) -> &'static str {
        match self {
            CollaborationError::NotFound => "not_found",
            CollaborationError::Validation(_) => "validation_error",
            CollaborationError::LimitExceeded(_) => "delegation_limit_exceeded",
            CollaborationError::Conflict(_) => "conflict",
            CollaborationError::Internal(_) => "internal_error",
        }
    }

    pub fn message(&self) -> String {
        match self {
            CollaborationError::NotFound => "Bot not found".into(),
            CollaborationError::Validation(m) => m.clone(),
            CollaborationError::LimitExceeded(m) => m.clone(),
            CollaborationError::Conflict(m) => m.clone(),
            CollaborationError::Internal(m) => m.clone(),
        }
    }
}

#[async_trait]
pub trait AgentCollaboration: Send + Sync {
    async fn list_bots(
        &self,
        ctx: &CollaborationContext,
    ) -> Result<Vec<BotTeammateSummary>, CollaborationError>;

    async fn delegate(
        &self,
        ctx: &CollaborationContext,
        target_bot_id: &str,
        instruction: &str,
        context: Option<&str>,
    ) -> Result<DelegationEnqueueResult, CollaborationError>;
}
