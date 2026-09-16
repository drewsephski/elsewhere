//! Provider-neutral durable Bot memory tools. Not a secret store.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub const MAX_MEMORY_CONTENT_BYTES: usize = 2048;
pub const MAX_MEMORY_RECALL_LIMIT: i64 = 8;
pub const DEFAULT_MEMORY_RECALL_LIMIT: i64 = 5;
pub const MAX_MEMORY_SEARCH_TERMS: usize = 8;
pub const MAX_MEMORY_SEARCH_TERM_BYTES: usize = 40;

pub const RECALL_MEMORY_TOOL_NAME: &str = "recall_memory";
pub const REMEMBER_TOOL_NAME: &str = "remember";
pub const FORGET_MEMORY_TOOL_NAME: &str = "forget_memory";

pub const RECALL_MEMORY_DESCRIPTION: &str = "Search this Bot's durable memories for facts, preferences, and working conventions. Use when the automatically retrieved remembered context may be incomplete. Read-only; never stores secrets.";

pub const REMEMBER_DESCRIPTION: &str = "Save a concise durable fact or preference for later work. Do not store passwords, API keys, tokens, cookies, one-time codes, or other secrets. Prefer standalone facts that will remain useful.";

pub const FORGET_MEMORY_DESCRIPTION: &str = "Forget a specific memory by id returned from recall_memory. Use when the owner says something is no longer true or should not be remembered.";

#[derive(Debug, Clone)]
pub struct MemoryContext {
    pub owner_id: String,
    pub bot_id: String,
    pub run_id: String,
    pub request_id: String,
    pub source_message_id: Option<String>,
    pub tool_invocation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryRecallItem {
    pub id: String,
    pub content: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryWriteResult {
    pub id: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryError {
    NotFound,
    Validation(String),
    Capacity(String),
    Internal(String),
}

impl MemoryError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::Validation(_) => "validation_error",
            Self::Capacity(_) => "memory_capacity",
            Self::Internal(_) => "internal_error",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::NotFound => "Memory not found".into(),
            Self::Validation(m) | Self::Capacity(m) | Self::Internal(m) => m.clone(),
        }
    }
}

#[async_trait]
pub trait AgentMemory: Send + Sync {
    async fn recall(
        &self,
        ctx: &MemoryContext,
        query: &str,
        limit: Option<i64>,
    ) -> Result<Vec<MemoryRecallItem>, MemoryError>;

    async fn remember(
        &self,
        ctx: &MemoryContext,
        content: &str,
        kind: Option<&str>,
    ) -> Result<MemoryWriteResult, MemoryError>;

    async fn forget(
        &self,
        ctx: &MemoryContext,
        memory_id: &str,
    ) -> Result<MemoryWriteResult, MemoryError>;
}

pub fn is_memory_mutation_tool(name: &str) -> bool {
    name == REMEMBER_TOOL_NAME || name == FORGET_MEMORY_TOOL_NAME
}
