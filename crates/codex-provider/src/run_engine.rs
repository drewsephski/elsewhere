use async_trait::async_trait;

use agent_core::{
    AgentLoopContext, RunEngine, RunEngineKind, SharedRunDeps,
};

pub struct CodexRunEngine;

impl CodexRunEngine {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RunEngine for CodexRunEngine {
    fn kind(&self) -> RunEngineKind {
        RunEngineKind::CodexSubscription
    }

    async fn run(
        &self,
        _ctx: AgentLoopContext,
        _shared: SharedRunDeps,
        _input: Vec<serde_json::Value>,
    ) -> Result<(), agent_core::RuntimeError> {
        Err(agent_core::RuntimeError::Model(
            "CodexRunEngine is not implemented yet (Phase 3B.2c)".into(),
        ))
    }
}
