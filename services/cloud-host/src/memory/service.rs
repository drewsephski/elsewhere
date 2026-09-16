use std::sync::Arc;

use agent_core::{
    AgentMemory, MemoryContext, MemoryError, MemoryRecallItem, MemoryWriteResult,
    DEFAULT_MEMORY_RECALL_LIMIT, MAX_MEMORY_RECALL_LIMIT,
};
use async_trait::async_trait;
use sqlx::PgPool;

use super::db::{archive_memory, get_memory_for_owner, insert_memory, search_memories, NewMemory};
use super::types::{MemoryKind, MemorySourceKind};
use crate::db::resources::get_bot_for_owner;
use crate::error::ApiError;

pub struct PostgresAgentMemory {
    pool: PgPool,
}

impl PostgresAgentMemory {
    pub fn new(pool: PgPool) -> Arc<Self> {
        Arc::new(Self { pool })
    }
}

fn map_api(err: ApiError) -> MemoryError {
    match err {
        ApiError::NotFound => MemoryError::NotFound,
        ApiError::Validation(m) if m.contains("already has") => MemoryError::Capacity(m),
        ApiError::Validation(m) => MemoryError::Validation(m),
        other => MemoryError::Internal(other.to_string()),
    }
}

#[async_trait]
impl AgentMemory for PostgresAgentMemory {
    async fn recall(
        &self,
        ctx: &MemoryContext,
        query: &str,
        limit: Option<i64>,
    ) -> Result<Vec<MemoryRecallItem>, MemoryError> {
        let bot = get_bot_for_owner(&self.pool, &ctx.owner_id, &ctx.bot_id)
            .await
            .map_err(|e| MemoryError::Internal(e.to_string()))?
            .ok_or(MemoryError::NotFound)?;
        if bot.id != ctx.bot_id {
            return Err(MemoryError::NotFound);
        }
        let limit = limit
            .unwrap_or(DEFAULT_MEMORY_RECALL_LIMIT)
            .clamp(1, MAX_MEMORY_RECALL_LIMIT);
        let records = search_memories(&self.pool, &ctx.owner_id, &ctx.bot_id, query, limit)
            .await
            .map_err(map_api)?;
        Ok(records
            .into_iter()
            .map(|record| MemoryRecallItem {
                id: record.id,
                content: record.content,
                kind: record.kind,
                source_kind: Some(record.source_kind),
                confirmed_at: Some(record.last_confirmed_at.to_rfc3339()),
            })
            .collect())
    }

    async fn remember(
        &self,
        ctx: &MemoryContext,
        content: &str,
        kind: Option<&str>,
    ) -> Result<MemoryWriteResult, MemoryError> {
        let kind = kind
            .map(MemoryKind::parse)
            .transpose()
            .map_err(MemoryError::Validation)?
            .unwrap_or(MemoryKind::Fact);
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| MemoryError::Internal(e.to_string()))?;
        let exists: Option<String> =
            sqlx::query_scalar("SELECT id FROM bots WHERE id = $1 AND owner_id = $2 FOR UPDATE")
                .bind(&ctx.bot_id)
                .bind(&ctx.owner_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| MemoryError::Internal(e.to_string()))?;
        if exists.is_none() {
            return Err(MemoryError::NotFound);
        }
        let record = insert_memory(
            &mut tx,
            NewMemory {
                owner_id: ctx.owner_id.clone(),
                bot_id: ctx.bot_id.clone(),
                kind,
                content: content.to_string(),
                search_terms: Vec::new(),
                importance: 3,
                confidence: 1.0,
                source_kind: MemorySourceKind::ExplicitTool,
                source_run_id: Some(ctx.run_id.clone()),
                source_message_id: ctx.source_message_id.clone(),
            },
        )
        .await
        .map_err(map_api)?;
        tx.commit()
            .await
            .map_err(|e| MemoryError::Internal(e.to_string()))?;
        Ok(MemoryWriteResult {
            id: record.id,
            status: record.status,
        })
    }

    async fn forget(
        &self,
        ctx: &MemoryContext,
        memory_id: &str,
    ) -> Result<MemoryWriteResult, MemoryError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| MemoryError::Internal(e.to_string()))?;
        let record = archive_memory(&mut tx, &ctx.owner_id, &ctx.bot_id, memory_id)
            .await
            .map_err(map_api)?;
        tx.commit()
            .await
            .map_err(|e| MemoryError::Internal(e.to_string()))?;
        Ok(MemoryWriteResult {
            id: record.id,
            status: record.status,
        })
    }
}

pub async fn require_owned_bot(
    pool: &PgPool,
    owner_id: &str,
    bot_id: &str,
) -> Result<(), ApiError> {
    get_bot_for_owner(pool, owner_id, bot_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::NotFound)?;
    Ok(())
}

pub async fn get_owned_memory(
    pool: &PgPool,
    owner_id: &str,
    bot_id: &str,
    memory_id: &str,
) -> Result<super::db::MemoryRecord, ApiError> {
    get_memory_for_owner(pool, owner_id, bot_id, memory_id)
        .await?
        .ok_or(ApiError::NotFound)
}
