//! Execution backend selection for Elsewhere (Phase 3B.2).
//!
//! Today all hosts use `ResponsesRunEngine` via `run_agent_loop` + `ResponsesModel`.
//! `CodexRunEngine` will drive runs through the Codex app-server while Elsewhere
//! supplies computer tools (MCP).

use std::sync::Arc;

use crate::computer::AgentComputer;
use crate::events::EventSink;
use crate::model::ResponsesModel;
use crate::run_store::RunStore;
use crate::runtime::{run_agent_loop, AgentLoopContext, AgentLoopDeps};
use std::sync::atomic::AtomicBool;

/// How the host executes an agent run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunEngineKind {
    /// OpenAI Responses API + `agent-core` tool loop (API key).
    ResponsesApi,
    /// Codex app-server + ChatGPT subscription (Codex owns the agent loop).
    CodexSubscription,
}

/// OpenAI API key path — existing Luna / Responses tool loop.
pub struct ResponsesRunEngine;

impl ResponsesRunEngine {
    pub async fn run(
        ctx: AgentLoopContext,
        deps: AgentLoopDeps,
        input: Vec<serde_json::Value>,
    ) -> Result<(), crate::events::RuntimeError> {
        run_agent_loop(ctx, deps, input).await
    }
}

/// Dependencies shared by any run engine implementation.
pub struct SharedRunDeps {
    pub computer: Arc<dyn AgentComputer>,
    pub store: Arc<dyn RunStore>,
    pub events: Arc<dyn EventSink>,
    pub cancel: Arc<AtomicBool>,
}

/// Codex subscription path (not wired yet).
pub struct CodexRunEngine;

impl CodexRunEngine {
    pub async fn run(
        _ctx: AgentLoopContext,
        _shared: SharedRunDeps,
        _input: Vec<serde_json::Value>,
    ) -> Result<(), crate::events::RuntimeError> {
        Err(crate::events::RuntimeError::Model(
            "CodexRunEngine is not implemented yet (Phase 3B.2)".into(),
        ))
    }
}

/// Build `AgentLoopDeps` for the Responses API engine.
pub fn responses_loop_deps(
    shared: SharedRunDeps,
    model: Arc<dyn ResponsesModel>,
) -> AgentLoopDeps {
    AgentLoopDeps {
        computer: shared.computer,
        store: shared.store,
        events: shared.events,
        model,
        cancel: shared.cancel,
    }
}
