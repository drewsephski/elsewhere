//! Structured owner questions that pause the current model turn (not a new run).

use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;

pub const ASK_USER_TOOL_NAME: &str = "ask_user";
pub const MAX_ASK_USER_QUESTION_CHARS: usize = 240;
pub const MAX_ASK_USER_OPTION_CHARS: usize = 80;
pub const MIN_ASK_USER_OPTIONS: usize = 2;
pub const MAX_ASK_USER_OPTIONS: usize = 4;
pub const MAX_ASK_USER_PER_RUN: usize = 3;

pub const ASK_USER_DESCRIPTION: &str = "\
Ask the owner one short multiple-choice question when the answer materially changes what you should do next, then continue this SAME assignment with the selected option. \
Prefer proceeding autonomously when the user's intent is already clear. Do not ask unnecessary confirmation questions. \
Do not use this tool to request passwords, API keys, OAuth codes, OTPs, payment credentials, or other secrets — use browser_request_human for protected browser input and normal approvals for dangerous mutations. \
ask_user is a decision/input, not permission and not a human-only browser step.";

const SECRET_SOLICITATION_MARKERS: &[&str] = &[
    "password",
    "passwd",
    "api key",
    "apikey",
    "api_key",
    "secret key",
    "oauth",
    "otp",
    "totp",
    "2fa",
    "two-factor",
    "two factor",
    "one-time code",
    "one time code",
    "credit card",
    "card number",
    "cvv",
    "ssn",
    "social security",
    "private key",
    "authorization code",
    "auth code",
    "passcode",
    "pin code",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserQuestionRequest {
    pub question: String,
    pub options: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserQuestionOutcome {
    pub question_id: String,
    pub selected_index: usize,
    pub selected_option: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserQuestionError {
    Validation(String),
    DuplicatePending,
    LimitReached,
    Cancelled,
    Interrupted,
    Internal(String),
}

#[derive(Debug, Clone)]
pub struct UserQuestionContext {
    pub owner_id: String,
    pub run_id: String,
    pub request_id: String,
    pub tool_invocation_id: String,
}

#[async_trait]
pub trait AgentUserQuestion: Send + Sync {
    async fn ask_and_wait(
        &self,
        ctx: &UserQuestionContext,
        request: &UserQuestionRequest,
        cancel: &AtomicBool,
    ) -> Result<UserQuestionOutcome, UserQuestionError>;
}

pub fn validate_user_question(
    question: &str,
    options: &[String],
) -> Result<UserQuestionRequest, UserQuestionError> {
    let question = collapse_whitespace(question);
    if question.is_empty() {
        return Err(UserQuestionError::Validation("question is required".into()));
    }
    if question.chars().count() > MAX_ASK_USER_QUESTION_CHARS {
        return Err(UserQuestionError::Validation(format!(
            "question exceeds {MAX_ASK_USER_QUESTION_CHARS} characters"
        )));
    }
    reject_secret_solicitation(&question)?;

    if options.len() < MIN_ASK_USER_OPTIONS || options.len() > MAX_ASK_USER_OPTIONS {
        return Err(UserQuestionError::Validation(format!(
            "options must contain {MIN_ASK_USER_OPTIONS} to {MAX_ASK_USER_OPTIONS} choices"
        )));
    }

    let mut normalized = Vec::new();
    let mut seen = HashSet::new();
    for option in options {
        let value = collapse_whitespace(option);
        if value.is_empty() {
            return Err(UserQuestionError::Validation(
                "options cannot be empty".into(),
            ));
        }
        if value.chars().count() > MAX_ASK_USER_OPTION_CHARS {
            return Err(UserQuestionError::Validation(format!(
                "each option must be at most {MAX_ASK_USER_OPTION_CHARS} characters"
            )));
        }
        reject_secret_solicitation(&value)?;
        let key = value.to_ascii_lowercase();
        if !seen.insert(key) {
            return Err(UserQuestionError::Validation(
                "options must be unique".into(),
            ));
        }
        normalized.push(value);
    }

    Ok(UserQuestionRequest {
        question,
        options: normalized,
    })
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn reject_secret_solicitation(value: &str) -> Result<(), UserQuestionError> {
    let lowered = value.to_ascii_lowercase();
    if SECRET_SOLICITATION_MARKERS
        .iter()
        .any(|marker| lowered.contains(marker))
    {
        return Err(UserQuestionError::Validation(
            "ask_user cannot request passwords, API keys, codes, or other secrets".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_question_and_options() {
        let request = validate_user_question(
            "  Which environment should I deploy to?  ",
            &["Staging".into(), "Production".into(), "Don't deploy".into()],
        )
        .unwrap();
        assert_eq!(request.question, "Which environment should I deploy to?");
        assert_eq!(request.options.len(), 3);
    }

    #[test]
    fn rejects_duplicate_and_secret_options() {
        assert!(validate_user_question("Choose", &["A".into()]).is_err());
        assert!(validate_user_question("Choose", &["A".into(), "a".into()]).is_err());
        assert!(
            validate_user_question("Send me the password", &["Yes".into(), "No".into()]).is_err()
        );
        assert!(validate_user_question(
            "Continue?",
            &["Paste the API key".into(), "Cancel".into()]
        )
        .is_err());
    }
}
