//! Portable Elsewhere agent runtime — no Tauri, SQLite, or VM dependencies.

mod approval;
mod browser_tools;
mod public_http_url;
mod computer;
mod events;
mod fake_computer;
mod input;
mod luna;
mod model;
mod run_engine;
mod runtime;
mod readiness_cache;
mod runtime_identity;
mod run_store;
mod collaboration;
mod collaboration_tools;
mod connectors;
mod connector_tools;
mod tool_catalog;
mod tools;
mod workspace_entries;

pub use approval::{
    approval_action_summary, operation_kind_for_tool, sanitize_tool_arguments,
    AllowAllApprovalGate, ApprovalDecision, ApprovalError, ToolApprovalContext, ToolRunContext,
    ToolApprovalGate, ToolOperationKind, MAX_EXEC_COMMAND_CHARS,
};
pub use computer::{
    AgentComputer, ComputerError, ComputerInfo, ExecResult, WorkspaceEntry, WorkspaceRevisionCounter,
    workspace_tool_mutation,
};
pub use events::{AgentEvent, EventSink, RuntimeError};
pub use input::{build_responses_input, ConversationMessage, MessageRole, MessageStatus};
pub use luna::{default_model_id, luna_model_id, DEFAULT_MODEL};
pub use model::{
    extract_assistant_text, extract_function_calls, function_call_output_item,
    model_supports_responses_tools, CreateResponseRequest, CreateResponseResult,
    ModelError, ResponsesModel,
};
pub use fake_computer::FakeAgentComputer;
pub use readiness_cache::ReadinessCachedComputer;
pub use runtime_identity::{compose_runtime_instruction_snapshot, RuntimeIdentityInput};
pub use public_http_url::validate_public_http_url;
pub use workspace_entries::{
    filter_workspace_listing, is_internal_workspace_entry, normalize_workspace_path,
    validate_workspace_mutation_path, validate_workspace_readable_path, workspace_rename_target,
};
pub use run_engine::{
    legacy_local_loop_deps, responses_loop_deps, ResponsesRunEngine, RunEngine, RunEngineKind,
    SharedRunDeps,
};
pub use runtime::{run_agent_loop, AgentLoopContext, AgentLoopDeps};
pub use tools::MAX_AGENT_TOOL_STEPS;
pub use run_store::{
    CreateRunParams, PersistedMessage, RunEventReceipt, RunStore, StructuredMessageInput,
};
pub use tools::{dispatch_tool, dispatch_tool_with_gate, openai_tool_definitions, ToolError};
pub use collaboration::{
    AgentCollaboration, BotTeammateSummary, CollaborationContext, CollaborationError,
    DelegationEnqueueResult, MAX_CHILD_DELEGATIONS_PER_ROOT, MAX_DELEGATION_CONTEXT_CHARS,
    MAX_DELEGATION_DEPTH, MAX_DELEGATION_INSTRUCTION_CHARS,
};
pub use collaboration_tools::{
    all_openai_tool_definitions, collaboration_openai_tool_definitions,
    dispatch_agent_tool_with_gate,
};
pub use connectors::{AgentConnectors, ConnectorError};
pub use connector_tools::connector_openai_tool_definitions;
pub use tool_catalog::{
    ALL_AGENT_TOOL_NAMES, ALL_COMPUTER_TOOL_NAMES, BROWSER_TOOL_NAMES, COLLABORATION_TOOL_NAMES,
    CONNECTOR_TOOL_NAMES, WORKSPACE_TOOL_NAMES, is_browser_tool, is_collaboration_tool,
    is_connector_tool,
};
