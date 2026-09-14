//! Portable Elsewhere agent runtime — no Tauri, SQLite, or VM dependencies.

mod computer;
mod events;
mod input;
mod luna;
mod model;
mod runtime;
mod run_store;
mod tools;

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
pub use runtime::{run_agent_loop, AgentLoopContext, AgentLoopDeps};
pub use tools::MAX_AGENT_TOOL_STEPS;
pub use run_store::{
    CreateRunParams, PersistedMessage, RunStore, StructuredMessageInput,
};
pub use tools::{dispatch_tool, openai_tool_definitions, ToolError};
