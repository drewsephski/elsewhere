use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use agent_core::{
    is_browser_mutation_tool, ApprovalDecision, ApprovalError, ToolApprovalContext,
    ToolApprovalGate,
};
use async_trait::async_trait;
use sqlx::PgPool;

use crate::approval::RunScopedApprovalGate;
use crate::computer_control;

/// Pauses bot browser mutations while the owner holds human control, then delegates to the run approval gate.
pub struct BrowserHumanControlGate {
    inner: Arc<dyn ToolApprovalGate>,
    pool: PgPool,
    cancel: Arc<AtomicBool>,
}

impl BrowserHumanControlGate {
    pub fn wrapping_run_gate(gate: RunScopedApprovalGate, pool: PgPool) -> Self {
        let cancel = gate.cancel.clone();
        Self {
            inner: Arc::new(gate),
            pool,
            cancel,
        }
    }

    pub fn wrapping_allow_all(pool: PgPool, cancel: Arc<AtomicBool>) -> Self {
        Self {
            inner: Arc::new(agent_core::AllowAllApprovalGate),
            pool,
            cancel,
        }
    }
}

#[async_trait]
impl ToolApprovalGate for BrowserHumanControlGate {
    async fn authorize(
        &self,
        context: &ToolApprovalContext,
    ) -> Result<ApprovalDecision, ApprovalError> {
        if is_browser_mutation_tool(&context.tool_name) {
            if self.cancel.load(Ordering::Relaxed) {
                return Err(ApprovalError::Cancelled);
            }
            computer_control::wait_for_bot_browser_control(
                &self.pool,
                &context.owner_id,
                &context.computer_id,
                &self.cancel,
            )
            .await?;
        }
        self.inner.authorize(context).await
    }
}
