use serde::{Deserialize, Serialize};

pub const MAX_ONBOARDING_QUESTIONS: i32 = 3;
pub const MAX_QUESTION_ID_CHARS: usize = 64;
pub const MAX_OPTION_ID_CHARS: usize = 64;
pub const MAX_HELPER_CHARS: usize = 200;
pub const MAX_OPTION_DESCRIPTION_CHARS: usize = 160;
pub const MAX_CUSTOM_ANSWER_CHARS: usize = 280;
pub const MAX_SUMMARY_CHARS: usize = 500;
pub const MAX_SUGGESTED_TASKS: usize = 3;
pub const MAX_SUGGESTED_TASK_CHARS: usize = 200;
pub const MAX_SUGGESTED_CAPABILITIES: usize = 5;
pub const MAX_SUGGESTED_CAPABILITY_CHARS: usize = 80;
pub const MAX_INSTRUCTIONS_BYTES: usize = 16_000;
pub const MAX_CONTEXT_BYTES: usize = 16_000;
pub const MAX_LAST_ERROR_CHARS: usize = 400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnboardingStatus {
    NotStarted,
    InProgress,
    ReadyToApply,
    Completed,
    Dismissed,
}

impl OnboardingStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotStarted => "not_started",
            Self::InProgress => "in_progress",
            Self::ReadyToApply => "ready_to_apply",
            Self::Completed => "completed",
            Self::Dismissed => "dismissed",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "not_started" => Some(Self::NotStarted),
            "in_progress" => Some(Self::InProgress),
            "ready_to_apply" => Some(Self::ReadyToApply),
            "completed" => Some(Self::Completed),
            "dismissed" => Some(Self::Dismissed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingOption {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingQuestion {
    pub id: String,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub helper: Option<String>,
    pub options: Vec<OnboardingOption>,
    #[serde(default)]
    pub allow_custom: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingAnswer {
    pub question_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub option_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_text: Option<String>,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingDraft {
    pub summary: String,
    #[serde(default)]
    pub owns: String,
    #[serde(default)]
    pub working_style: String,
    pub instructions: String,
    pub context: String,
    #[serde(default)]
    pub suggested_first_tasks: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggested_capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum OnboardingModelResponse {
    Question {
        question: OnboardingQuestion,
    },
    Complete {
        #[serde(flatten)]
        draft: OnboardingDraft,
    },
}

impl OnboardingModelResponse {
    pub fn into_draft(self) -> Option<OnboardingDraft> {
        match self {
            Self::Complete { draft } => Some(draft),
            Self::Question { .. } => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OnboardingGenerateInput {
    pub bot_name: String,
    pub bot_instructions: String,
    pub bot_model: String,
    pub pinned_context: String,
    pub answers: Vec<OnboardingAnswer>,
    pub questions_asked: i32,
    pub generation_model: String,
}

#[derive(Debug, Clone)]
pub struct BotOnboardingRecord {
    pub bot_id: String,
    pub owner_id: String,
    pub status: OnboardingStatus,
    pub questions_asked: i32,
    pub current_question: Option<OnboardingQuestion>,
    pub answers: Vec<OnboardingAnswer>,
    pub draft: Option<OnboardingDraft>,
    pub last_error: Option<String>,
    pub revision: i64,
}
