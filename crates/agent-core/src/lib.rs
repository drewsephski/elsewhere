//! Portable Elsewhere agent runtime — no Tauri, SQLite, or VM dependencies.

mod approval;
mod computer;
mod events;
mod fake_computer;
mod input;
mod luna;
mod model;
mod run_engine;
mod runtime;
mod run_store;
mod tools;

pub use approval::{
    approval_action_summary, operation_kind_for_tool, sanitize_tool_arguments,
    AllowAllApprovalGate, ApprovalDecision, ApprovalError, ToolApprovalContext, ToolRunContext,
    ToolApprovalGate, ToolOperationKind, MAX_EXEC_COMMAND_CHARS,
};
pub use computer::{
    AgentComputer, ComputerError, ComputerInfo, ExecResult, WorkspaceEntry,
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
