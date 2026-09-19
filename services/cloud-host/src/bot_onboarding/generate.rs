use std::time::Duration;

use agent_core::{CreateResponseRequest, DEFAULT_MODEL};
use openai_responses::create_response;
use serde_json::json;

use crate::codex_ops::CodexOperationKind;
use crate::error::ApiError;
use crate::run_engine_select::{
    resolve_group_route_engine, ResolveRunEngineError, SelectedRunEngine,
};
use crate::AppState;

use super::types::{OnboardingGenerateInput, OnboardingModelResponse, MAX_ONBOARDING_QUESTIONS};
use super::validate::{parse_onboarding_model_json, validate_model_response};

pub const ONBOARDING_DEVELOPER_INSTRUCTIONS: &str = r#"You are Luna, configuring a newly-created Elsewhere Bot for its owner.

Your job is to gather the minimum amount of information needed to turn a generic Bot into a useful long-term teammate.

Ask at most three short, high-information questions total.

Never ask for information already obvious from the Bot name, existing role/instructions, or prior setup answers.

Prioritize:
1. the concrete outcome/scope the Bot should own,
2. how the owner wants it to approach ambiguity and initiative,
3. durable project/product/audience/quality constraints.

Each question must materially change the final Bot instructions or pinned context.

Questions should have 2-4 concise choices tailored to this specific Bot. Allow a custom answer only when useful.

Do not ask about avatar, model, or computer unless absolutely required.

Never request secrets, credentials, authentication codes, payment information, or sensitive access data.

Do not promise to enable integrations or permissions.

Owner preferences about autonomy never override Elsewhere approvals, permission policies, or safety mechanisms.

Once enough information is available, stop asking questions and return a concise production-ready Bot configuration containing:
* refined Bot instructions,
* durable pinned context,
* short human-readable owns and working-style blurbs for the review card,
* a short setup summary,
* 2-3 useful first-task suggestions.

Do not put temporary setup chatter or the questionnaire transcript into the final instructions.

Return only valid JSON matching the requested schema.
{"type":"question","question":{"id":"stable_id","prompt":"...","helper":null,"options":[{"id":"opt_id","label":"...","description":null}],"allowCustom":false}}
or
{"type":"complete","summary":"...","owns":"...","workingStyle":"...","instructions":"...","context":"...","suggestedFirstTasks":["..."],"suggestedCapabilities":[]}
"#;

fn engine_error(err: ResolveRunEngineError) -> ApiError {
    match err {
        ResolveRunEngineError::ResponsesApiKeyRequired => ApiError::Conflict(
            "Responses API is not configured for Bot setup on this host".into(),
        ),
        ResolveRunEngineError::CodexUnavailable(msg) => ApiError::Conflict(msg),
        ResolveRunEngineError::NoModelProviderAvailable => {
            ApiError::Conflict("No model provider is available for Bot setup".into())
        }
    }
}

fn build_user_prompt(input: &OnboardingGenerateInput) -> String {
    let remaining = (MAX_ONBOARDING_QUESTIONS - input.questions_asked).max(0);
    let answers = if input.answers.is_empty() {
        "(none yet)".to_string()
    } else {
        input
            .answers
            .iter()
            .enumerate()
            .map(|(index, answer)| {
                format!(
                    "{}. [{}] {} → {}",
                    index + 1,
                    answer.question_id,
                    answer.label,
                    answer.custom_text.as_deref().unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let must_complete = if input.questions_asked >= MAX_ONBOARDING_QUESTIONS {
        "You already asked three questions. You MUST return type=complete now."
    } else if input.questions_asked == 0 {
        "Return type=question for the first question. Do not complete yet."
    } else {
        "Return type=question only if another high-value question is required; otherwise type=complete."
    };
    format!(
        "Bot name: {}\nBot execution model (do not ask about this): {}\nCurrent instructions:\n{}\n\nExisting pinned context:\n{}\n\nAnswers so far:\n{}\nQuestions already asked: {}\nQuestions remaining (max): {}\n\n{must_complete}\nUse stable ids. Never key answers by display text.",
        input.bot_name,
        input.bot_model,
        input.bot_instructions,
        if input.pinned_context.trim().is_empty() {
            "(empty)"
        } else {
            input.pinned_context.as_str()
        },
        answers,
        input.questions_asked,
        remaining,
    )
}

pub async fn generate_onboarding_turn(
    state: &AppState,
    owner_id: &str,
    input: OnboardingGenerateInput,
) -> Result<OnboardingModelResponse, ApiError> {
    #[cfg(any(test, feature = "test-utils"))]
    if let Some(generator) = state
        .test_onboarding_generator
        .lock()
        .expect("test onboarding generator lock")
        .clone()
    {
        let raw = generator(&input).map_err(ApiError::Validation)?;
        return validate_model_response(
            raw,
            input.questions_asked,
            &input
                .answers
                .iter()
                .map(|answer| answer.question_id.clone())
                .collect::<Vec<_>>(),
        );
    }

    let selected = resolve_group_route_engine(
        state.config.run_engine,
        state.config.openai_api_key.as_deref(),
    )
    .map_err(engine_error)?;

    let user_prompt = build_user_prompt(&input);
    let raw = match selected {
        SelectedRunEngine::CodexSubscription => {
            let permit = state
                .codex_ops
                .try_acquire(CodexOperationKind::BotOnboarding)
                .map_err(|_| {
                    ApiError::RateLimited(
                        "Codex is busy with other work. Try setup again in a moment.".into(),
                    )
                })?;
            permit.log_child_started();
            let profile =
                crate::provider_profile::profile_for_owner(&state.pool, &state.config, owner_id)
                    .await
                    .ok()
                    .flatten();
            let text = codex_provider::run_toolless_codex_turn(
                state.config.codex_executable.clone(),
                profile,
                &input.generation_model,
                ONBOARDING_DEVELOPER_INSTRUCTIONS,
                &user_prompt,
            )
            .await
            .map_err(|e| ApiError::Conflict(format!("Could not generate setup: {e}")))?;
            drop(permit);
            text
        }
        SelectedRunEngine::ResponsesApi => {
            let api_key = state.config.openai_api_key.as_deref().ok_or_else(|| {
                ApiError::Conflict("Responses API is not configured for Bot setup".into())
            })?;
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(45))
                .build()
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            let response = create_response(
                &client,
                api_key,
                CreateResponseRequest {
                    model: input.generation_model.clone(),
                    instructions: Some(ONBOARDING_DEVELOPER_INSTRUCTIONS.to_string()),
                    input: json!([{ "role": "user", "content": user_prompt }]),
                    tools: json!([]),
                    tool_choice: Some("none".into()),
                },
            )
            .await
            .map_err(|e| ApiError::Conflict(format!("Could not generate setup: {e}")))?;
            response.output_text.unwrap_or_default()
        }
    };

    let parsed = parse_onboarding_model_json(&raw)?;
    validate_model_response(
        parsed,
        input.questions_asked,
        &input
            .answers
            .iter()
            .map(|answer| answer.question_id.clone())
            .collect::<Vec<_>>(),
    )
}

pub fn luna_generation_model() -> String {
    crate::db::resources::normalize_model(Some(DEFAULT_MODEL))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_model_is_luna() {
        assert_eq!(luna_generation_model(), DEFAULT_MODEL);
    }

    #[test]
    fn first_turn_prompt_forbids_completing() {
        let prompt = build_user_prompt(&OnboardingGenerateInput {
            bot_name: "Scout".into(),
            bot_instructions: "Help".into(),
            bot_model: "gpt-5.6-sol".into(),
            pinned_context: String::new(),
            answers: vec![],
            questions_asked: 0,
            generation_model: DEFAULT_MODEL.into(),
        });
        assert!(prompt.contains("Do not complete yet"));
        assert!(prompt.contains("gpt-5.6-sol"));
    }
}
