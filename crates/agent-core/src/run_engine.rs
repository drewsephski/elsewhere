//! Execution backend selection for Elsewhere (Phase 3B.2).
//!
//! `ResponsesRunEngine` drives the OpenAI Responses API tool loop today.
//! Codex subscription runs live in `codex-provider::CodexRunEngine`.

use async_trait::async_trait;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::approval::{AllowAllApprovalGate, ToolApprovalGate};
use crate::browser_recovery::BrowserRecoverySession;
use crate::collaboration::AgentCollaboration;
use crate::computer::AgentComputer;
use crate::connectors::AgentConnectors;
use crate::github_coding::AgentGithubCoding;
use crate::events::{EventSink, RuntimeError};
use crate::model::ResponsesModel;
use crate::run_store::RunStore;
use crate::run_user_input::RunUserInput;
use crate::runtime::{run_agent_loop, AgentLoopContext, AgentLoopDeps};
use agent_skills::SkillPackage;

/// How the host executes an agent run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunEngineKind {
    /// OpenAI Responses API + `agent-core` tool loop (API key).
    ResponsesApi,
    /// Codex app-server + ChatGPT subscription (Codex owns the agent loop).
    CodexSubscription,
}

/// Host-facing execution backend.
#[async_trait]
pub trait RunEngine: Send + Sync {
    fn kind(&self) -> RunEngineKind;

    async fn run(
        &self,
        ctx: AgentLoopContext,
        deps: SharedRunDeps,
        input: Vec<serde_json::Value>,
    ) -> Result<(), RuntimeError>;
}

/// Dependencies shared by any run engine implementation.
pub struct SharedRunDeps {
    pub computer: Arc<dyn AgentComputer>,
    pub store: Arc<dyn RunStore>,
    pub events: Arc<dyn EventSink>,
    pub cancel: Arc<AtomicBool>,
    pub approval_gate: Arc<dyn ToolApprovalGate>,
    pub run_id: String,
    pub owner_id: String,
    pub computer_id: String,
    pub collaboration: Option<Arc<dyn AgentCollaboration>>,
    pub connectors: Option<Arc<dyn AgentConnectors>>,
    pub human_intervention: Option<Arc<dyn crate::human_intervention::AgentHumanIntervention>>,
    pub browser_recovery: Option<Arc<BrowserRecoverySession>>,
    pub subagents: Option<Arc<dyn crate::subagent::AgentSubagents>>,
    pub memory: Option<Arc<dyn crate::memory::AgentMemory>>,
    pub routines: Option<Arc<dyn crate::routines::AgentRoutines>>,
    pub skills: Option<Arc<dyn crate::skills::AgentSkills>>,
    pub github_coding: Option<Arc<dyn AgentGithubCoding>>,
    pub attachments: Option<Arc<dyn crate::attachments::AgentAttachments>>,
    pub user_questions: Option<Arc<dyn crate::user_question::AgentUserQuestion>>,
    /// Provider-neutral user text plus immutable attachment descriptors for this run.
    pub user_input: RunUserInput,
    /// Immutable Agent Skill packages snapshotted at run admission.
    pub skill_packages: Arc<[SkillPackage]>,
}

impl SharedRunDeps {
    pub fn allow_all_approval(
        computer: Arc<dyn AgentComputer>,
        store: Arc<dyn RunStore>,
        events: Arc<dyn EventSink>,
        cancel: Arc<AtomicBool>,
        run_id: String,
        owner_id: String,
        computer_id: String,
    ) -> Self {
        Self {
            computer,
            store,
            events,
            cancel,
            approval_gate: Arc::new(AllowAllApprovalGate),
            run_id,
            owner_id,
            computer_id,
            collaboration: None,
            connectors: None,
            human_intervention: None,
            browser_recovery: None,
            subagents: None,
            memory: None,
            routines: None,
            skills: None,
            github_coding: None,
            attachments: None,
            user_questions: None,
            user_input: RunUserInput::from_text(""),
            skill_packages: Arc::from([]),
        }
    }
}

/// OpenAI API key path — existing Luna / Responses tool loop.
pub struct ResponsesRunEngine {
    model: Arc<dyn ResponsesModel>,
}

impl ResponsesRunEngine {
    pub fn new(model: Arc<dyn ResponsesModel>) -> Self {
        Self { model }
    }
}

#[async_trait]
impl RunEngine for ResponsesRunEngine {
    fn kind(&self) -> RunEngineKind {
        RunEngineKind::ResponsesApi
    }

    async fn run(
        &self,
        ctx: AgentLoopContext,
        deps: SharedRunDeps,
        input: Vec<serde_json::Value>,
    ) -> Result<(), RuntimeError> {
        let loop_deps = responses_loop_deps(deps, self.model.clone());
        run_agent_loop(ctx, loop_deps, input).await
    }
}

/// Build `AgentLoopDeps` for the Responses API engine.
pub fn responses_loop_deps(shared: SharedRunDeps, model: Arc<dyn ResponsesModel>) -> AgentLoopDeps {
    AgentLoopDeps {
        computer: shared.computer,
        store: shared.store,
        events: shared.events,
        model,
        cancel: shared.cancel,
        approval_gate: shared.approval_gate.clone(),
        run_id: shared.run_id.clone(),
        owner_id: shared.owner_id.clone(),
        computer_id: shared.computer_id.clone(),
        collaboration: shared.collaboration.clone(),
        connectors: shared.connectors.clone(),
        human_intervention: shared.human_intervention.clone(),
        browser_recovery: shared.browser_recovery.clone(),
        subagents: shared.subagents.clone(),
        memory: shared.memory.clone(),
        routines: shared.routines.clone(),
        skills: shared.skills.clone(),
        github_coding: shared.github_coding.clone(),
        attachments: shared.attachments.clone(),
        user_questions: shared.user_questions.clone(),
    }
}

/// Local/demo `AgentLoopDeps` with automatic tool approval.
pub fn legacy_local_loop_deps(
    computer: Arc<dyn AgentComputer>,
    store: Arc<dyn RunStore>,
    events: Arc<dyn EventSink>,
    model: Arc<dyn ResponsesModel>,
    cancel: Arc<AtomicBool>,
    run_id: impl Into<String>,
    computer_id: impl Into<String>,
) -> AgentLoopDeps {
    responses_loop_deps(
        SharedRunDeps::allow_all_approval(
            computer,
            store,
            events,
            cancel,
            run_id.into(),
            "legacy-local".into(),
            computer_id.into(),
        ),
        model,
    )
}
