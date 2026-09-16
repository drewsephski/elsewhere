//! Portable Elsewhere agent runtime — no Tauri, SQLite, or VM dependencies.

mod approval;
mod browser_recovery;
mod browser_tools;
mod collaboration;
mod collaboration_tools;
mod computer;
mod connector_tools;
mod connectors;
mod events;
mod fake_computer;
mod human_intervention;
mod human_intervention_tools;
mod input;
mod luna;
mod model;
mod public_http_url;
mod readiness_cache;
mod run_engine;
mod run_store;
mod runtime;
mod runtime_identity;
mod subagent;
mod tool_catalog;
mod tools;
mod workspace_entries;

pub use approval::{
    approval_action_summary, operation_kind_for_tool, sanitize_tool_arguments,
    AllowAllApprovalGate, ApprovalDecision, ApprovalError, ToolApprovalContext, ToolApprovalGate,
    ToolOperationKind, ToolRunContext, MAX_EXEC_COMMAND_CHARS,
};
pub use browser_recovery::{
    browser_recovery_policy_instructions, classify_recoverable_failure, detect_human_blocker,
    plan_recovery_after_failure, BrowserRecoverySession, HumanBlockerHint, RecoverableFailureKind,
    RecoveryHint, MAX_RECOVERY_ATTEMPTS,
};
pub use collaboration::{
    AgentCollaboration, BotTeammateSummary, CollaborationContext, CollaborationError,
    DelegationEnqueueResult, MAX_CHILD_DELEGATIONS_PER_ROOT, MAX_DELEGATION_CONTEXT_CHARS,
    MAX_DELEGATION_DEPTH, MAX_DELEGATION_INSTRUCTION_CHARS,
};
pub use collaboration_tools::{
    all_openai_tool_definitions, collaboration_openai_tool_definitions,
    dispatch_agent_tool_with_gate, dispatch_agent_tool_with_gate_and_recovery,
};
pub use computer::{
    workspace_tool_mutation, AgentComputer, ComputerError, ComputerInfo, ExecResult,
    WorkspaceEntry, WorkspaceRevisionCounter,
};
pub use connector_tools::connector_openai_tool_definitions;
pub use connectors::{
    bound_connector_tool_result, AgentConnectors, ConnectorError, MAX_CONNECTOR_TOOL_RESULT_BYTES,
};
pub use events::{AgentEvent, EventSink, RuntimeError};
pub use fake_computer::FakeAgentComputer;
pub use human_intervention::{
    is_human_intervention_tool, sanitize_human_intervention_message,
    validate_human_intervention_reason, AgentHumanIntervention, HumanInterventionContext,
    HumanInterventionError, HumanInterventionOutcome, HUMAN_INTERVENTION_REASONS,
    MAX_HUMAN_INTERVENTION_MESSAGE_CHARS,
};
pub use human_intervention_tools::human_intervention_openai_tool_definitions;
pub use input::{build_responses_input, ConversationMessage, MessageRole, MessageStatus};
pub use luna::{default_model_id, luna_model_id, DEFAULT_MODEL};
pub use model::{
    extract_assistant_text, extract_function_calls, function_call_output_item,
    model_supports_responses_tools, CreateResponseRequest, CreateResponseResult, ModelError,
    ResponsesModel,
};
pub use public_http_url::validate_public_http_url;
pub use readiness_cache::ReadinessCachedComputer;
pub use run_engine::{
    legacy_local_loop_deps, responses_loop_deps, ResponsesRunEngine, RunEngine, RunEngineKind,
    SharedRunDeps,
};
pub use run_store::{
    CreateRunParams, PersistedMessage, RunEventReceipt, RunStore, StructuredMessageInput,
};
pub use runtime::{run_agent_loop, AgentLoopContext, AgentLoopDeps};
pub use runtime_identity::{compose_runtime_instruction_snapshot, RuntimeIdentityInput};
pub use subagent::{
    bound_subagent_result, subagent_developer_instructions, subagent_task_summary,
    subagent_user_prompt, truncate_utf8_bytes, validate_subagent_request, AgentSubagents,
    InMemoryAgentSubagents, ResponsesSubagentTurn, SubagentContext, SubagentError, SubagentRequest,
    SubagentResult, SubagentTurn, MAX_ACTIVE_SUBAGENTS_PER_PARENT, MAX_SUBAGENTS_PER_PARENT,
    MAX_SUBAGENT_CONTEXT_CHARS, MAX_SUBAGENT_NAME_CHARS, MAX_SUBAGENT_RESULT_BYTES,
    MAX_SUBAGENT_TASK_CHARS, MAX_SUBAGENT_TASK_SUMMARY_CHARS, RUN_SUBAGENT_DESCRIPTION,
    RUN_SUBAGENT_TOOL_NAME, SUBAGENT_TURN_TIMEOUT_SECS,
};
pub use tool_catalog::{
    is_browser_mutation_tool, is_browser_tool, is_collaboration_tool, is_connector_tool,
    is_known_agent_tool, is_policy_non_overridable_tool, is_policy_overridable_tool,
    is_subagent_tool, policy_action_group, policy_action_label, policy_denied_message,
    PolicyActionGroup, ALL_AGENT_TOOL_NAMES, ALL_COMPUTER_TOOL_NAMES, BROWSER_TOOL_NAMES,
    COLLABORATION_TOOL_NAMES, CONNECTOR_TOOL_NAMES, POLICY_NON_OVERRIDABLE_TOOL_NAMES,
    POLICY_OVERRIDABLE_TOOL_NAMES, SUBAGENT_TOOL_NAMES, WORKSPACE_TOOL_NAMES,
};
pub use tools::MAX_AGENT_TOOL_STEPS;
pub use tools::{dispatch_tool, dispatch_tool_with_gate, openai_tool_definitions, ToolError};
pub use workspace_entries::{
    filter_workspace_listing, is_internal_workspace_entry, normalize_workspace_path,
    validate_workspace_mutation_path, validate_workspace_readable_path, workspace_rename_target,
};
