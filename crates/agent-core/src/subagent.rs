//! Provider-neutral ephemeral subagents. Not Bots, computers, or queued agent runs.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::events::RuntimeError;
use crate::model::{extract_assistant_text, CreateResponseRequest, ModelError, ResponsesModel};

pub const MAX_SUBAGENTS_PER_PARENT: i64 = 4;
pub const MAX_ACTIVE_SUBAGENTS_PER_PARENT: i64 = 1;
pub const MAX_SUBAGENT_NAME_CHARS: usize = 64;
pub const MAX_SUBAGENT_TASK_CHARS: usize = 8_000;
pub const MAX_SUBAGENT_CONTEXT_CHARS: usize = 4_000;
pub const MAX_SUBAGENT_RESULT_BYTES: usize = 12 * 1024;
pub const SUBAGENT_TURN_TIMEOUT_SECS: u64 = 120;
pub const MAX_SUBAGENT_TASK_SUMMARY_CHARS: usize = 80;

pub const RUN_SUBAGENT_TOOL_NAME: &str = "run_subagent";

pub const RUN_SUBAGENT_DESCRIPTION: &str = "Run a short-lived helper inside this assignment and wait for its findings. Use for focused reasoning, analysis, summarization, planning, critique, comparison, or other work that does not require an independent persistent teammate. Use bot_delegate when another real Bot should own work asynchronously or needs its own computer. Subagents v1 have NO computer, browser, shell, filesystem, connectors, delegation tools, credentials, MCP tools, or nested subagents. Do not copy the entire transcript; supply the task and only the relevant context.";

#[derive(Debug, Clone)]
pub struct SubagentContext {
    pub owner_id: String,
    pub bot_id: String,
    pub parent_run_id: String,
    pub parent_request_id: String,
    pub tool_invocation_id: String,
    pub model: String,
    pub cancel: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
pub struct SubagentRequest {
    pub name: String,
    pub task: String,
    pub context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubagentResult {
    pub subagent_id: String,
    pub name: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubagentError {
    Validation(String),
    LimitExceeded(String),
    Unsupported(String),
    Cancelled,
    Internal(String),
}

impl SubagentError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Validation(_) => "validation_error",
            Self::LimitExceeded(_) => "subagent_limit_exceeded",
            Self::Unsupported(_) => "subagent_unsupported",
            Self::Cancelled => "cancelled",
            Self::Internal(_) => "internal_error",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::Validation(m)
            | Self::LimitExceeded(m)
            | Self::Unsupported(m)
            | Self::Internal(m) => m.clone(),
            Self::Cancelled => "Agent run cancelled".into(),
        }
    }
}

#[async_trait]
pub trait SubagentTurn: Send + Sync {
    async fn run_toolless(
        &self,
        model: &str,
        developer_instructions: &str,
        user_prompt: &str,
        cancel: &AtomicBool,
    ) -> Result<String, SubagentError>;
}

#[async_trait]
pub trait AgentSubagents: Send + Sync {
    fn attach_turn_executor(&self, executor: Arc<dyn SubagentTurn>) {
        let _ = executor;
    }

    fn clear_turn_executor(&self) {}

    async fn run_subagent(
        &self,
        ctx: &SubagentContext,
        request: SubagentRequest,
    ) -> Result<SubagentResult, SubagentError>;
}

pub struct ResponsesSubagentTurn {
    model: Arc<dyn ResponsesModel>,
}

impl ResponsesSubagentTurn {
    pub fn new(model: Arc<dyn ResponsesModel>) -> Self {
        Self { model }
    }
}

#[async_trait]
impl SubagentTurn for ResponsesSubagentTurn {
    async fn run_toolless(
        &self,
        model: &str,
        developer_instructions: &str,
        user_prompt: &str,
        cancel: &AtomicBool,
    ) -> Result<String, SubagentError> {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(SubagentError::Cancelled);
        }
        let response = self
            .model
            .create_response(CreateResponseRequest {
                model: model.to_string(),
                instructions: Some(developer_instructions.to_string()),
                input: serde_json::json!([{
                    "role": "user",
                    "content": user_prompt
                }]),
                tools: serde_json::json!([]),
                tool_choice: None,
            })
            .await
            .map_err(map_model_error)?;
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(SubagentError::Cancelled);
        }
        let text = extract_assistant_text(&response.output, response.output_text.as_deref());
        if text.trim().is_empty() {
            return Err(SubagentError::Internal(
                "subagent produced empty assistant text".into(),
            ));
        }
        Ok(bound_subagent_result(&text))
    }
}

fn map_model_error(err: ModelError) -> SubagentError {
    match err {
        ModelError::Cancelled => SubagentError::Cancelled,
        other => SubagentError::Internal(other.to_string()),
    }
}

pub struct InMemoryAgentSubagents {
    turn: RwLock<Option<Arc<dyn SubagentTurn>>>,
}

impl InMemoryAgentSubagents {
    pub fn new() -> Self {
        Self {
            turn: RwLock::new(None),
        }
    }
}

impl Default for InMemoryAgentSubagents {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentSubagents for InMemoryAgentSubagents {
    fn attach_turn_executor(&self, executor: Arc<dyn SubagentTurn>) {
        *self.turn.write().expect("subagent turn lock") = Some(executor);
    }

    fn clear_turn_executor(&self) {
        *self.turn.write().expect("subagent turn lock") = None;
    }

    async fn run_subagent(
        &self,
        ctx: &SubagentContext,
        request: SubagentRequest,
    ) -> Result<SubagentResult, SubagentError> {
        let validated = validate_subagent_request(&request)?;
        let executor = self
            .turn
            .read()
            .expect("subagent turn lock")
            .clone()
            .ok_or_else(|| {
                SubagentError::Unsupported(
                    "A subagent cannot run because this assignment has no helper executor.".into(),
                )
            })?;
        let developer = subagent_developer_instructions();
        let user_prompt = subagent_user_prompt(
            &validated.name,
            &validated.task,
            validated.context.as_deref(),
        );
        let text = executor
            .run_toolless(&ctx.model, &developer, &user_prompt, ctx.cancel.as_ref())
            .await?;
        Ok(SubagentResult {
            subagent_id: format!("memory:{}", ctx.tool_invocation_id),
            name: validated.name,
            status: "completed".into(),
            result: Some(text),
            error: None,
        })
    }
}

pub fn validate_subagent_request(
    request: &SubagentRequest,
) -> Result<SubagentRequest, SubagentError> {
    let name = request.name.trim();
    if name.is_empty() {
        return Err(SubagentError::Validation(
            "Subagent `name` is required.".into(),
        ));
    }
    if name.chars().count() > MAX_SUBAGENT_NAME_CHARS {
        return Err(SubagentError::Validation(format!(
            "Subagent name must be at most {MAX_SUBAGENT_NAME_CHARS} characters."
        )));
    }
    let task = request.task.trim();
    if task.is_empty() {
        return Err(SubagentError::Validation(
            "Subagent `task` is required.".into(),
        ));
    }
    if task.chars().count() > MAX_SUBAGENT_TASK_CHARS {
        return Err(SubagentError::Validation(format!(
            "Subagent task must be at most {MAX_SUBAGENT_TASK_CHARS} characters."
        )));
    }
    let context = request
        .context
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    if let Some(ctx) = &context {
        if ctx.chars().count() > MAX_SUBAGENT_CONTEXT_CHARS {
            return Err(SubagentError::Validation(format!(
                "Subagent context must be at most {MAX_SUBAGENT_CONTEXT_CHARS} characters."
            )));
        }
    }
    Ok(SubagentRequest {
        name: name.to_string(),
        task: task.to_string(),
        context,
    })
}

pub fn subagent_developer_instructions() -> String {
    "You are an ephemeral helper working for a parent Bot in Elsewhere. \
You are not a Bot, you have no conversation, and you have no computer. \
Complete only the supplied task. Use supplied context as supporting data. \
Return concise findings the parent Bot can use. \
Never claim you performed browser, filesystem, shell, account, or connector actions. \
You have no tools. Do not mention secrets, credentials, or unrelated conversation history."
        .into()
}

pub fn subagent_user_prompt(name: &str, task: &str, context: Option<&str>) -> String {
    let mut prompt = format!("Helper name: {name}\n\nTask:\n{task}\n");
    if let Some(context) = context.filter(|value| !value.trim().is_empty()) {
        prompt.push_str("\nSupporting context:\n");
        prompt.push_str(context);
        prompt.push('\n');
    } else {
        prompt.push_str("\nNo additional context was supplied.\n");
    }
    prompt
}

pub fn subagent_task_summary(task: &str) -> String {
    let trimmed = task.trim();
    if trimmed.chars().count() <= MAX_SUBAGENT_TASK_SUMMARY_CHARS {
        return trimmed.to_string();
    }
    let mut summary: String = trimmed
        .chars()
        .take(MAX_SUBAGENT_TASK_SUMMARY_CHARS)
        .collect();
    summary.push('…');
    summary
}

pub fn bound_subagent_result(text: &str) -> String {
    truncate_utf8_bytes(text, MAX_SUBAGENT_RESULT_BYTES)
}

pub fn truncate_utf8_bytes(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let ellipsis = "…";
    if max_bytes <= ellipsis.len() {
        return ellipsis.to_string();
    }
    let budget = max_bytes - ellipsis.len();
    let mut end = budget;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{ellipsis}", &text[..end])
}

impl From<RuntimeError> for SubagentError {
    fn from(err: RuntimeError) -> Self {
        SubagentError::Internal(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_name_and_task_bounds() {
        let err = validate_subagent_request(&SubagentRequest {
            name: String::new(),
            task: "review".into(),
            context: None,
        })
        .unwrap_err();
        assert!(matches!(err, SubagentError::Validation(_)));

        let long_task = "a".repeat(MAX_SUBAGENT_TASK_CHARS + 1);
        let err = validate_subagent_request(&SubagentRequest {
            name: "Reviewer".into(),
            task: long_task,
            context: None,
        })
        .unwrap_err();
        assert!(matches!(err, SubagentError::Validation(_)));
    }

    #[test]
    fn helper_prompt_does_not_claim_tools() {
        let developer = subagent_developer_instructions();
        assert!(developer.contains("ephemeral helper"));
        assert!(developer.contains("no tools"));
        assert!(developer.contains("Never claim you performed browser"));
        let prompt = subagent_user_prompt("Reviewer", "check the plan", Some("diff summary"));
        assert!(prompt.contains("Helper name: Reviewer"));
        assert!(prompt.contains("diff summary"));
        assert!(!prompt.contains("run_subagent"));
    }
}
