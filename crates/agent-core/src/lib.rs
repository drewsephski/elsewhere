//! Portable Elsewhere agent runtime — no Tauri, SQLite, or VM dependencies.

mod approval;
mod attachment_tools;
mod attachments;
mod browser_recovery;
mod browser_tools;
mod collaboration;
mod collaboration_tools;
mod computer;
mod connector_tools;
mod connectors;
mod github_coding;
mod github_coding_tools;
mod events;
mod fake_computer;
mod human_intervention;
mod human_intervention_tools;
mod input;
mod local_mac_protocol;
mod luna;
mod memory;
mod memory_tools;
mod routine_tools;
mod routines;
mod skill_tools;
mod skills;
mod model;
mod public_http_url;
mod readiness_cache;
mod run_engine;
mod run_store;
mod run_user_input;
mod runtime;
mod runtime_identity;
mod subagent;
mod tool_catalog;
mod tools;
mod user_question;
mod user_question_tools;
mod workspace_entries;

pub use approval::{
    approval_action_summary, operation_kind_for_tool, sanitize_tool_arguments,
    AllowAllApprovalGate, ApprovalDecision, ApprovalError, ConnectedAppApprovalInfo,
    ToolApprovalContext, ToolApprovalGate, ToolOperationKind, ToolRunContext,
    MAX_EXEC_COMMAND_CHARS,
};
pub use attachment_tools::{
    attachment_openai_tool_definitions, dispatch_attachment_tool, ATTACHMENT_LIST_DESCRIPTION,
    ATTACHMENT_LIST_TOOL_NAME, ATTACHMENT_READ_DESCRIPTION, ATTACHMENT_READ_TOOL_NAME,
};
pub use attachments::{
    AgentAttachments, AttachmentError, AttachmentTextPage, InMemoryAttachments,
    MAX_ATTACHMENT_READ_BYTES,
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
pub use github_coding::{
    is_github_coding_mutation_tool, AgentGithubCoding, GithubCodingError, GITHUB_CODING_TOOL_NAMES,
    GITHUB_OPEN_REPOSITORY_TOOL, GITHUB_PUBLISH_PULL_REQUEST_TOOL, GITHUB_REVIEW_PUBLISH_TOOL,
    GITHUB_RUN_CHECK_TOOL,
};
pub use github_coding_tools::{
    dispatch_github_coding_tool, github_coding_openai_tool_definitions,
};
pub use connectors::{
    bound_connector_tool_result, truncate_connector_tool_result, AgentConnectors, ConnectorError,
    ConnectorToolDefinition, ConnectorToolRoute, CONNECTED_APPS_EXECUTE_TOOL,
    CONNECTED_APPS_LOAD_TOOL, CONNECTED_APPS_SEARCH_TOOL, MAX_CONNECTED_APP_DESCRIPTION_CHARS,
    MAX_CONNECTED_APP_SEARCH_LIMIT, MAX_CONNECTOR_TOOL_RESULT_BYTES,
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
pub use local_mac_protocol::{
    computer_error_from_rpc, computer_info_from_result, decode_bytes, encode_bytes,
    exec_result_from_result, is_mutation_method, read_file_bytes_from_result,
    rpc_error_from_computer, workspace_entries_from_result, ExecParams, HostToMacMessage,
    LocalMacRpcDispatcher, MacToHostMessage, PathParams, ReadFileResult, RpcError, WriteFileParams,
    AMBIGUOUS_MUTATION_MESSAGE, CONTENT_ENCODING_BASE64, LOCAL_MAC_NOT_CONNECTED,
    METHOD_ENSURE_READY, METHOD_EXEC, METHOD_LIST_DIR, METHOD_READ_FILE, METHOD_WRITE_FILE,
    PROTOCOL_VERSION,
};
pub use luna::{
    canonical_bot_model, default_model_id, luna_model_id, BotModelSpec, BOT_MODELS, DEFAULT_MODEL,
};
pub use memory::{
    is_memory_mutation_tool, AgentMemory, MemoryContext, MemoryError, MemoryRecallItem,
    MemoryWriteResult, DEFAULT_MEMORY_RECALL_LIMIT, FORGET_MEMORY_DESCRIPTION,
    FORGET_MEMORY_TOOL_NAME, MAX_MEMORY_CONTENT_BYTES, MAX_MEMORY_RECALL_LIMIT,
    MAX_MEMORY_SEARCH_TERMS, MAX_MEMORY_SEARCH_TERM_BYTES, RECALL_MEMORY_DESCRIPTION,
    RECALL_MEMORY_TOOL_NAME, REMEMBER_DESCRIPTION, REMEMBER_TOOL_NAME,
};
pub use memory_tools::{dispatch_memory_tool, memory_openai_tool_definitions};
pub use routine_tools::{dispatch_routine_tool, routine_openai_tool_definitions};
pub use routines::{
    is_routine_mutation_tool, AgentRoutines, BotRoutineSchedule, RoutineContext,
    RoutineCreateDraft, RoutineError, RoutineMutationResult, RoutineSummary,
    ROUTINE_CREATE_TOOL_NAME,
    ROUTINE_LIST_TOOL_NAME, ROUTINE_PAUSE_TOOL_NAME, ROUTINE_RESUME_TOOL_NAME,
};
pub use skill_tools::{
    dispatch_skill_tool, skill_openai_tool_definitions, truncate_skill_md_preview,
};
pub use skills::{
    is_skill_mutation_tool, AgentSkills, SkillAttachResult, SkillContext, SkillDetachResult,
    SkillDraftFile, SkillError, SkillListEntry, SkillSaveDraft, SkillSaveResult,
    SKILL_ATTACH_TOOL_NAME, SKILL_DETACH_TOOL_NAME, SKILL_LIST_TOOL_NAME,
    SKILL_SAVE_RECENT_WORK_TOOL_NAME,
};
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
pub use run_user_input::{
    attachment_workspace_path, AttachmentDescriptor, AttachmentKind, RunUserInput,
    ATTACHMENT_SAFETY_CONTRACT, MAX_ATTACHMENTS_PER_MESSAGE, MAX_ATTACHMENT_BYTES,
    MAX_ATTACHMENT_TOTAL_BYTES, MAX_INLINE_DOCUMENT_BYTES, STAGED_ATTACHMENT_TTL_HOURS,
    UNTRUSTED_DOCUMENT_PREFACE,
};
pub use runtime::{run_agent_loop, AgentLoopContext, AgentLoopDeps};
pub use runtime_identity::{
    compose_runtime_instruction_snapshot, RuntimeIdentityInput, RuntimeMemoryFact,
};
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
    is_attachment_tool, is_browser_mutation_tool, is_browser_tool, is_collaboration_tool,
    is_connected_apps_execute_tool, is_connected_apps_tool, is_connector_tool,
    is_github_coding_tool, is_github_connector_tool, is_known_agent_tool, is_memory_tool,
    is_policy_non_overridable_tool,
    is_policy_overridable_tool, is_routine_tool, is_skill_tool, is_subagent_tool,
    is_user_question_tool,
    policy_action_group,
    policy_action_label, policy_denied_message, PolicyActionGroup, ALL_AGENT_TOOL_NAMES,
    ALL_COMPUTER_TOOL_NAMES, ATTACHMENT_TOOL_NAMES, BROWSER_TOOL_NAMES, COLLABORATION_TOOL_NAMES,
    CONNECTED_APPS_TOOL_NAMES, CONNECTOR_TOOL_NAMES, MEMORY_TOOL_NAMES, ROUTINE_TOOL_NAMES,
    POLICY_NON_OVERRIDABLE_TOOL_NAMES, POLICY_OVERRIDABLE_TOOL_NAMES, SUBAGENT_TOOL_NAMES,
    USER_QUESTION_TOOL_NAMES, WORKSPACE_TOOL_NAMES,
};
pub use tools::MAX_AGENT_TOOL_STEPS;
pub use tools::{dispatch_tool, dispatch_tool_with_gate, openai_tool_definitions, ToolError};
pub use user_question::{
    validate_user_question, AgentUserQuestion, UserQuestionContext, UserQuestionError,
    UserQuestionOutcome, UserQuestionRequest, ASK_USER_DESCRIPTION, ASK_USER_TOOL_NAME,
    MAX_ASK_USER_OPTIONS, MAX_ASK_USER_OPTION_CHARS, MAX_ASK_USER_PER_RUN,
    MAX_ASK_USER_QUESTION_CHARS, MIN_ASK_USER_OPTIONS,
};
pub use user_question_tools::{dispatch_user_question_tool, user_question_openai_tool_definitions};
pub use workspace_entries::{
    filter_workspace_listing, is_hidden_workspace_listing_name, is_internal_workspace_entry,
    is_user_visible_workspace_entry, normalize_workspace_path, validate_workspace_list_path,
    validate_workspace_mutation_path, validate_workspace_readable_path, workspace_rename_target,
};
