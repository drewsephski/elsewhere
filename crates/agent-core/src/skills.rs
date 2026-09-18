//! Bot-facing Agent Skill tools (list, save recent work, attach, detach).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub const SKILL_LIST_TOOL_NAME: &str = "skill_list";
pub const SKILL_SAVE_RECENT_WORK_TOOL_NAME: &str = "skill_save_recent_work";
pub const SKILL_ATTACH_TOOL_NAME: &str = "skill_attach";
pub const SKILL_DETACH_TOOL_NAME: &str = "skill_detach";

pub const SKILL_LIST_DESCRIPTION: &str =
    "List Agent Skills owned by this user, including whether each is attached to this Bot. Read-only.";

pub const SKILL_SAVE_RECENT_WORK_DESCRIPTION: &str =
    "Turn the most recent completed assignment in this conversation (before this save request) into a reusable Agent Skill. Requires owner approval. Optionally set name, description, and whether to attach the skill to this Bot.";

pub const SKILL_ATTACH_DESCRIPTION: &str =
    "Attach an existing Agent Skill to this Bot so it can be invoked with /skill-name. Requires owner approval.";

pub const SKILL_DETACH_DESCRIPTION: &str =
    "Remove an Agent Skill from this Bot. Requires owner approval.";

#[derive(Debug, Clone)]
pub struct SkillContext {
    pub owner_id: String,
    pub bot_id: String,
    pub source_conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillListEntry {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub status: String,
    pub current_version: i32,
    pub attached_to_bot: bool,
    pub pinned_version: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillSaveDraft {
    pub source_run_id: String,
    pub slug: String,
    pub display_name: String,
    pub description: String,
    pub skill_md: String,
    pub attach_to_bot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillSaveResult {
    pub skill_id: String,
    pub slug: String,
    pub name: String,
    pub version: i32,
    pub attached_to_bot: bool,
    pub bot_name: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillAttachResult {
    pub skill_id: String,
    pub slug: String,
    pub name: String,
    pub bot_name: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillDetachResult {
    pub skill_id: String,
    pub slug: String,
    pub name: String,
    pub bot_name: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillError {
    NotFound,
    NoSourceRun,
    Validation(String),
    Conflict(String),
    Forbidden,
    Internal(String),
}

impl SkillError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::NoSourceRun => "no_source_run",
            Self::Validation(_) => "validation_error",
            Self::Conflict(_) => "conflict",
            Self::Forbidden => "forbidden",
            Self::Internal(_) => "internal_error",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::NotFound => "Skill not found".into(),
            Self::NoSourceRun => {
                "No recent completed assignment found in this conversation to save as a skill.".into()
            }
            Self::Validation(m) | Self::Conflict(m) | Self::Internal(m) => m.clone(),
            Self::Forbidden => "You cannot change this skill".into(),
        }
    }
}

#[async_trait]
pub trait AgentSkills: Send + Sync {
    async fn list(&self, ctx: &SkillContext) -> Result<Vec<SkillListEntry>, SkillError>;

    async fn prepare_save_from_recent_work(
        &self,
        ctx: &SkillContext,
        current_run_id: &str,
        optional_name: Option<&str>,
        optional_description: Option<&str>,
        attach_to_bot: bool,
    ) -> Result<SkillSaveDraft, SkillError>;

    async fn persist_save(
        &self,
        ctx: &SkillContext,
        draft: &SkillSaveDraft,
    ) -> Result<SkillSaveResult, SkillError>;

    async fn attach(
        &self,
        ctx: &SkillContext,
        skill_id: &str,
    ) -> Result<SkillAttachResult, SkillError>;

    async fn detach(
        &self,
        ctx: &SkillContext,
        skill_id: &str,
    ) -> Result<SkillDetachResult, SkillError>;

    async fn bot_display_name(&self, ctx: &SkillContext) -> Result<String, SkillError>;
}

pub fn is_skill_mutation_tool(name: &str) -> bool {
    matches!(
        name,
        SKILL_SAVE_RECENT_WORK_TOOL_NAME | SKILL_ATTACH_TOOL_NAME | SKILL_DETACH_TOOL_NAME
    )
}
