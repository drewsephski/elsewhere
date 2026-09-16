//! Owner handoff when the Bot cannot complete a browser step alone.

use async_trait::async_trait;
use std::sync::atomic::AtomicBool;

pub const HUMAN_INTERVENTION_REASONS: &[&str] = &[
    "login",
    "captcha",
    "two_factor",
    "passkey",
    "credentials",
    "consent",
    "other",
];

pub const MAX_HUMAN_INTERVENTION_MESSAGE_CHARS: usize = 500;

#[derive(Debug, Clone)]
pub struct HumanInterventionContext {
    pub owner_id: String,
    pub run_id: String,
    pub request_id: String,
    pub computer_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HumanInterventionError {
    Validation(String),
    DuplicatePending,
    Cancelled,
    Internal(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanInterventionOutcome {
    pub intervention_id: String,
}

#[async_trait]
pub trait AgentHumanIntervention: Send + Sync {
    async fn request_and_wait(
        &self,
        ctx: &HumanInterventionContext,
        reason: &str,
        message: &str,
        cancel: &AtomicBool,
    ) -> Result<HumanInterventionOutcome, HumanInterventionError>;
}

pub fn is_human_intervention_tool(name: &str) -> bool {
    name == "browser_request_human"
}

pub fn validate_human_intervention_reason(reason: &str) -> Result<(), HumanInterventionError> {
    if HUMAN_INTERVENTION_REASONS.contains(&reason) {
        Ok(())
    } else {
        Err(HumanInterventionError::Validation(format!(
            "reason must be one of: {}",
            HUMAN_INTERVENTION_REASONS.join(", ")
        )))
    }
}

pub fn sanitize_human_intervention_message(
    message: &str,
) -> Result<String, HumanInterventionError> {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return Err(HumanInterventionError::Validation(
            "message is required".into(),
        ));
    }
    let collapsed = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.len() > MAX_HUMAN_INTERVENTION_MESSAGE_CHARS {
        return Err(HumanInterventionError::Validation(format!(
            "message exceeds {MAX_HUMAN_INTERVENTION_MESSAGE_CHARS} characters"
        )));
    }
    Ok(collapsed)
}
