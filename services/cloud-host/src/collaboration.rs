//! Postgres-backed `AgentCollaboration` for cloud-host runs.

use std::sync::Arc;

use agent_core::{
    AgentCollaboration, BotTeammateSummary, CollaborationContext, CollaborationError,
    DelegationEnqueueResult,
};
use async_trait::async_trait;
use sqlx::PgPool;

pub struct PostgresAgentCollaboration {
    pool: PgPool,
}

impl PostgresAgentCollaboration {
    pub fn new(pool: PgPool) -> Arc<Self> {
        Arc::new(Self { pool })
    }
}

#[async_trait]
impl AgentCollaboration for PostgresAgentCollaboration {
    async fn list_bots(
        &self,
        ctx: &CollaborationContext,
    ) -> Result<Vec<BotTeammateSummary>, CollaborationError> {
        crate::delegation::list_teammates(&self.pool, &ctx.owner_id, &ctx.source_bot_id).await
    }

    async fn delegate(
        &self,
        ctx: &CollaborationContext,
        target_bot_id: &str,
        instruction: &str,
        context: Option<&str>,
        return_policy: &str,
    ) -> Result<DelegationEnqueueResult, CollaborationError> {
        crate::delegation::create_delegation(
            &self.pool,
            ctx,
            target_bot_id,
            instruction,
            context,
            return_policy,
        )
        .await
    }
}
