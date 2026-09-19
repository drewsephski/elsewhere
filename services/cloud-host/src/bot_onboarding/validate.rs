use std::collections::HashSet;

use agent_core::{
    text_solicits_secrets, MAX_ASK_USER_OPTION_CHARS, MAX_ASK_USER_OPTIONS,
    MAX_ASK_USER_QUESTION_CHARS, MIN_ASK_USER_OPTIONS,
};

use crate::error::ApiError;
use crate::memory::secrets::looks_like_secret;

use super::types::{
    OnboardingAnswer, OnboardingDraft, OnboardingModelResponse, OnboardingQuestion,
    MAX_CONTEXT_BYTES, MAX_CUSTOM_ANSWER_CHARS, MAX_HELPER_CHARS, MAX_INSTRUCTIONS_BYTES,
    MAX_ONBOARDING_QUESTIONS, MAX_OPTION_DESCRIPTION_CHARS, MAX_OPTION_ID_CHARS,
    MAX_QUESTION_ID_CHARS, MAX_SUGGESTED_CAPABILITIES, MAX_SUGGESTED_CAPABILITY_CHARS,
    MAX_SUGGESTED_TASK_CHARS, MAX_SUGGESTED_TASKS, MAX_SUMMARY_CHARS,
};

pub fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn reject_secrets(value: &str, what: &str) -> Result<(), ApiError> {
    if text_solicits_secrets(value) || looks_like_secret(value) {
        return Err(ApiError::Validation(format!(
            "{what} cannot include passwords, API keys, codes, or other secrets"
        )));
    }
    Ok(())
}

fn validate_stable_id(value: &str, what: &str, max_chars: usize) -> Result<String, ApiError> {
    let id = collapse_whitespace(value);
    if id.is_empty() || id.chars().count() > max_chars {
        return Err(ApiError::Validation(format!(
            "{what} id must contain 1 to {max_chars} characters"
        )));
    }
    if !id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err(ApiError::Validation(format!(
            "{what} id must be alphanumeric with hyphens or underscores"
        )));
    }
    Ok(id)
}

pub fn validate_question(question: OnboardingQuestion) -> Result<OnboardingQuestion, ApiError> {
    let id = validate_stable_id(&question.id, "question", MAX_QUESTION_ID_CHARS)?;
    let prompt = collapse_whitespace(&question.prompt);
    if prompt.is_empty() {
        return Err(ApiError::Validation("question prompt is required".into()));
    }
    if prompt.chars().count() > MAX_ASK_USER_QUESTION_CHARS {
        return Err(ApiError::Validation(format!(
            "question prompt exceeds {MAX_ASK_USER_QUESTION_CHARS} characters"
        )));
    }
    reject_secrets(&prompt, "Question")?;

    let helper = question
        .helper
        .as_deref()
        .map(collapse_whitespace)
        .filter(|value| !value.is_empty());
    if let Some(helper) = helper.as_deref() {
        if helper.chars().count() > MAX_HELPER_CHARS {
            return Err(ApiError::Validation(format!(
                "question helper exceeds {MAX_HELPER_CHARS} characters"
            )));
        }
        reject_secrets(helper, "Question")?;
    }

    if question.options.len() < MIN_ASK_USER_OPTIONS
        || question.options.len() > MAX_ASK_USER_OPTIONS
    {
        return Err(ApiError::Validation(format!(
            "each question must include {MIN_ASK_USER_OPTIONS} to {MAX_ASK_USER_OPTIONS} choices"
        )));
    }

    let mut options = Vec::new();
    let mut seen_ids = HashSet::new();
    let mut seen_labels = HashSet::new();
    for option in question.options {
        let option_id = validate_stable_id(&option.id, "option", MAX_OPTION_ID_CHARS)?;
        if !seen_ids.insert(option_id.to_ascii_lowercase()) {
            return Err(ApiError::Validation("option ids must be unique".into()));
        }
        let label = collapse_whitespace(&option.label);
        if label.is_empty() || label.chars().count() > MAX_ASK_USER_OPTION_CHARS {
            return Err(ApiError::Validation(format!(
                "each option label must contain 1 to {MAX_ASK_USER_OPTION_CHARS} characters"
            )));
        }
        reject_secrets(&label, "Option")?;
        if !seen_labels.insert(label.to_ascii_lowercase()) {
            return Err(ApiError::Validation("option labels must be unique".into()));
        }
        let description = option
            .description
            .as_deref()
            .map(collapse_whitespace)
            .filter(|value| !value.is_empty());
        if let Some(description) = description.as_deref() {
            if description.chars().count() > MAX_OPTION_DESCRIPTION_CHARS {
                return Err(ApiError::Validation(format!(
                    "option descriptions must be at most {MAX_OPTION_DESCRIPTION_CHARS} characters"
                )));
            }
            reject_secrets(description, "Option")?;
        }
        options.push(super::types::OnboardingOption {
            id: option_id,
            label,
            description,
        });
    }

    Ok(OnboardingQuestion {
        id,
        prompt,
        helper,
        options,
        allow_custom: question.allow_custom,
    })
}

pub fn validate_draft(draft: OnboardingDraft) -> Result<OnboardingDraft, ApiError> {
    let summary = collapse_whitespace(&draft.summary);
    if summary.is_empty() || summary.chars().count() > MAX_SUMMARY_CHARS {
        return Err(ApiError::Validation(format!(
            "setup summary must contain 1 to {MAX_SUMMARY_CHARS} characters"
        )));
    }
    reject_secrets(&summary, "Setup summary")?;

    let owns = collapse_whitespace(&draft.owns);
    if owns.chars().count() > MAX_SUMMARY_CHARS {
        return Err(ApiError::Validation(format!(
            "setup ownership blurb must be at most {MAX_SUMMARY_CHARS} characters"
        )));
    }
    if !owns.is_empty() {
        reject_secrets(&owns, "Setup ownership")?;
    }

    let working_style = collapse_whitespace(&draft.working_style);
    if working_style.chars().count() > MAX_SUMMARY_CHARS {
        return Err(ApiError::Validation(format!(
            "setup working-style blurb must be at most {MAX_SUMMARY_CHARS} characters"
        )));
    }
    if !working_style.is_empty() {
        reject_secrets(&working_style, "Setup working style")?;
    }

    let instructions = draft.instructions.trim().to_string();
    if instructions.is_empty() || instructions.len() > MAX_INSTRUCTIONS_BYTES {
        return Err(ApiError::Validation(
            "generated instructions must contain 1 to 16,000 bytes".into(),
        ));
    }
    reject_secrets(&instructions, "Bot instructions")?;

    let context = draft.context.trim().to_string();
    if context.len() > MAX_CONTEXT_BYTES {
        return Err(ApiError::Validation(
            "generated context must be at most 16,000 bytes".into(),
        ));
    }
    if !context.is_empty() {
        reject_secrets(&context, "Bot context")?;
    }

    if draft.suggested_first_tasks.len() > MAX_SUGGESTED_TASKS {
        return Err(ApiError::Validation(format!(
            "at most {MAX_SUGGESTED_TASKS} first-task suggestions are allowed"
        )));
    }
    let mut tasks = Vec::new();
    for task in draft.suggested_first_tasks {
        let value = collapse_whitespace(&task);
        if value.is_empty() {
            continue;
        }
        if value.chars().count() > MAX_SUGGESTED_TASK_CHARS {
            return Err(ApiError::Validation(format!(
                "first-task suggestions must be at most {MAX_SUGGESTED_TASK_CHARS} characters"
            )));
        }
        reject_secrets(&value, "Suggested task")?;
        tasks.push(value);
    }

    if draft.suggested_capabilities.len() > MAX_SUGGESTED_CAPABILITIES {
        return Err(ApiError::Validation(format!(
            "at most {MAX_SUGGESTED_CAPABILITIES} capability suggestions are allowed"
        )));
    }
    let mut capabilities = Vec::new();
    for capability in draft.suggested_capabilities {
        let value = collapse_whitespace(&capability);
        if value.is_empty() {
            continue;
        }
        if value.chars().count() > MAX_SUGGESTED_CAPABILITY_CHARS {
            return Err(ApiError::Validation(format!(
                "capability suggestions must be at most {MAX_SUGGESTED_CAPABILITY_CHARS} characters"
            )));
        }
        reject_secrets(&value, "Suggested capability")?;
        capabilities.push(value);
    }

    Ok(OnboardingDraft {
        summary,
        owns,
        working_style,
        instructions,
        context,
        suggested_first_tasks: tasks,
        suggested_capabilities: capabilities,
    })
}

pub fn validate_model_response(
    response: OnboardingModelResponse,
    questions_asked: i32,
    previous_question_ids: &[String],
) -> Result<OnboardingModelResponse, ApiError> {
    match response {
        OnboardingModelResponse::Question { question } => {
            if questions_asked >= MAX_ONBOARDING_QUESTIONS {
                return Err(ApiError::Validation(
                    "setup already asked the maximum number of questions".into(),
                ));
            }
            let question = validate_question(question)?;
            if previous_question_ids
                .iter()
                .any(|id| id.eq_ignore_ascii_case(&question.id))
            {
                return Err(ApiError::Validation(
                    "question ids must be unique across setup".into(),
                ));
            }
            Ok(OnboardingModelResponse::Question { question })
        }
        OnboardingModelResponse::Complete { draft } => {
            if questions_asked < 1 {
                return Err(ApiError::Validation(
                    "setup must ask at least one question before completing".into(),
                ));
            }
            let draft = validate_draft(draft)?;
            Ok(OnboardingModelResponse::Complete { draft })
        }
    }
}

pub fn parse_onboarding_model_json(raw: &str) -> Result<OnboardingModelResponse, ApiError> {
    let trimmed = raw.trim();
    let json_slice = if let Some(start) = trimmed.find('{') {
        let end = trimmed.rfind('}').ok_or_else(|| {
            ApiError::Validation("setup returned no JSON object".into())
        })?;
        &trimmed[start..=end]
    } else {
        return Err(ApiError::Validation(
            "setup returned no JSON object".into(),
        ));
    };
    serde_json::from_str(json_slice)
        .map_err(|e| ApiError::Validation(format!("invalid setup JSON: {e}")))
}

pub fn validate_onboarding_answer(
    question: &OnboardingQuestion,
    option_id: Option<&str>,
    custom_text: Option<&str>,
) -> Result<OnboardingAnswer, ApiError> {
    if let Some(option_id) = option_id.map(str::trim).filter(|value| !value.is_empty()) {
        let option = question
            .options
            .iter()
            .find(|option| option.id == option_id)
            .ok_or_else(|| ApiError::Validation("that choice is not available".into()))?;
        return Ok(OnboardingAnswer {
            question_id: question.id.clone(),
            option_id: Some(option.id.clone()),
            custom_text: None,
            label: option.label.clone(),
        });
    }

    if !question.allow_custom {
        return Err(ApiError::Validation("pick one of the listed choices".into()));
    }
    let custom = collapse_whitespace(custom_text.unwrap_or(""));
    if custom.is_empty() {
        return Err(ApiError::Validation("write a short custom answer".into()));
    }
    if custom.chars().count() > MAX_CUSTOM_ANSWER_CHARS {
        return Err(ApiError::Validation(format!(
            "custom answers must be at most {MAX_CUSTOM_ANSWER_CHARS} characters"
        )));
    }
    reject_secrets(&custom, "Custom answer")?;
    Ok(OnboardingAnswer {
        question_id: question.id.clone(),
        option_id: None,
        custom_text: Some(custom.clone()),
        label: custom,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bot_onboarding::types::OnboardingOption;

    fn question() -> OnboardingQuestion {
        OnboardingQuestion {
            id: "scope".into(),
            prompt: "What should Scout own?".into(),
            helper: None,
            options: vec![
                OnboardingOption {
                    id: "research".into(),
                    label: "Research briefs".into(),
                    description: None,
                },
                OnboardingOption {
                    id: "ops".into(),
                    label: "Launch ops".into(),
                    description: None,
                },
            ],
            allow_custom: true,
        }
    }

    #[test]
    fn rejects_secret_questions_and_answers() {
        let mut q = question();
        q.prompt = "Paste the API key".into();
        assert!(validate_question(q).is_err());
        assert!(validate_onboarding_answer(&question(), None, Some("password=hunter2")).is_err());
    }

    #[test]
    fn accepts_custom_answer() {
        let answer = validate_onboarding_answer(&question(), None, Some("Own competitor briefs")).unwrap();
        assert_eq!(answer.custom_text.as_deref(), Some("Own competitor briefs"));
        assert!(answer.option_id.is_none());
    }

    #[test]
    fn forbids_a_fourth_question() {
        let response = OnboardingModelResponse::Question {
            question: question(),
        };
        assert!(validate_model_response(response, 3, &[]).is_err());
    }

    #[test]
    fn complete_requires_at_least_one_answer() {
        let response = OnboardingModelResponse::Complete {
            draft: OnboardingDraft {
                summary: "Ready".into(),
                owns: "Competitor research briefs".into(),
                working_style: "Draft first, then check before publishing.".into(),
                instructions: "Do careful research.".into(),
                context: "Audience is founders.".into(),
                suggested_first_tasks: vec!["Write a one-page brief".into()],
                suggested_capabilities: vec![],
            },
        };
        assert!(validate_model_response(response.clone(), 0, &[]).is_err());
        assert!(validate_model_response(response, 1, &[]).is_ok());
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(parse_onboarding_model_json("not json").is_err());
        assert!(parse_onboarding_model_json("{\"type\":\"nope\"}").is_err());
    }

    #[test]
    fn parses_complete_json_with_flattened_fields() {
        let parsed = parse_onboarding_model_json(
            r#"{"type":"complete","summary":"Ready","owns":"Research","workingStyle":"Ask first","instructions":"Do careful research.","context":"Founders.","suggestedFirstTasks":["Write a brief"]}"#,
        )
        .unwrap();
        let draft = parsed.into_draft().unwrap();
        assert_eq!(draft.owns, "Research");
        assert_eq!(draft.working_style, "Ask first");
        assert_eq!(draft.suggested_first_tasks, vec!["Write a brief"]);
    }
}
