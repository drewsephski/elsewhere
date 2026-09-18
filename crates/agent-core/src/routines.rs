//! Provider-neutral recurring work (Routines) tools for Bots in conversation.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub const ROUTINE_LIST_TOOL_NAME: &str = "routine_list";
pub const ROUTINE_CREATE_TOOL_NAME: &str = "routine_create";
pub const ROUTINE_PAUSE_TOOL_NAME: &str = "routine_pause";
pub const ROUTINE_RESUME_TOOL_NAME: &str = "routine_resume";

pub const ROUTINE_LIST_DESCRIPTION: &str =
    "List this Bot's scheduled routines for the owner. Read-only. Use when the user asks what recurring work you have, or before pausing or resuming a routine by name.";

pub const ROUTINE_CREATE_DESCRIPTION: &str =
    "Create a new scheduled routine for this Bot. Requires owner approval. Use human-friendly schedules (every N minutes, daily at a time, weekdays at a time, or weekly on selected days). Do not use cron here — cron is only for the advanced Routines page.";

pub const ROUTINE_PAUSE_DESCRIPTION: &str =
    "Pause a routine so it stops running until resumed. Requires owner approval. Use routine_list to find the routine id when needed.";

pub const ROUTINE_RESUME_DESCRIPTION: &str =
    "Resume a paused routine. Requires owner approval.";

pub const ROUTINE_TOOL_NAMES: &[&str] = &[
    ROUTINE_LIST_TOOL_NAME,
    ROUTINE_CREATE_TOOL_NAME,
    ROUTINE_PAUSE_TOOL_NAME,
    ROUTINE_RESUME_TOOL_NAME,
];

#[derive(Debug, Clone)]
pub struct RoutineContext {
    pub owner_id: String,
    pub bot_id: String,
    pub source_conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BotRoutineSchedule {
    /// `every_minutes` | `daily` | `weekdays` | `weekly`
    pub repeat: String,
    /// Required for `every_minutes` (15–43200).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub every_minutes: Option<i32>,
    /// Local time `HH:MM` for daily, weekdays, and weekly schedules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    /// For `weekly`: `weekdays`, `MON,TUE,...`, or preset strings like `MON,WED,FRI`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoutineSummary {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub schedule_label: String,
    pub timezone: String,
    pub next_run_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoutineMutationResult {
    pub routine: RoutineSummary,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutineError {
    NotFound,
    Validation(String),
    Forbidden,
    Internal(String),
}

impl RoutineError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::Validation(_) => "validation_error",
            Self::Forbidden => "forbidden",
            Self::Internal(_) => "internal_error",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::NotFound => "Routine not found".into(),
            Self::Validation(m) | Self::Internal(m) => m.clone(),
            Self::Forbidden => "You cannot change this routine".into(),
        }
    }
}

#[async_trait]
pub trait AgentRoutines: Send + Sync {
    async fn list(&self, ctx: &RoutineContext) -> Result<Vec<RoutineSummary>, RoutineError>;

    async fn create(
        &self,
        ctx: &RoutineContext,
        name: &str,
        instructions: &str,
        schedule: &BotRoutineSchedule,
        timezone: &str,
        destination_conversation_id: Option<&str>,
    ) -> Result<RoutineMutationResult, RoutineError>;

    async fn set_enabled(
        &self,
        ctx: &RoutineContext,
        routine_id: &str,
        enabled: bool,
    ) -> Result<RoutineMutationResult, RoutineError>;
}

pub fn is_routine_mutation_tool(name: &str) -> bool {
    matches!(
        name,
        ROUTINE_CREATE_TOOL_NAME | ROUTINE_PAUSE_TOOL_NAME | ROUTINE_RESUME_TOOL_NAME
    )
}
