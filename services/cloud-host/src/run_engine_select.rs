use std::env;

use codex_provider::CodexSubscriptionAvailability;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunEngineMode {
    Codex,
    Responses,
    Auto,
}

impl RunEngineMode {
    pub fn from_env() -> Result<Self, String> {
        let raw = env::var("ELSEWHERE_RUN_ENGINE").unwrap_or_else(|_| "auto".into());
        parse_run_engine_mode(&raw)
    }
}

pub fn parse_run_engine_mode(raw: &str) -> Result<RunEngineMode, String> {
    match raw.to_ascii_lowercase().as_str() {
        "codex" => Ok(RunEngineMode::Codex),
        "responses" | "openai" | "api" => Ok(RunEngineMode::Responses),
        "auto" => Ok(RunEngineMode::Auto),
        other => Err(format!(
            "invalid ELSEWHERE_RUN_ENGINE value '{other}'; expected codex, responses, or auto"
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedRunEngine {
    CodexSubscription,
    ResponsesApi,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveRunEngineError {
    CodexUnavailable(String),
    ResponsesApiKeyRequired,
    NoModelProviderAvailable,
}

impl ResolveRunEngineError {
    pub fn as_run_message(&self) -> String {
        match self {
            Self::CodexUnavailable(msg) => msg.clone(),
            Self::ResponsesApiKeyRequired => {
                "OPENAI_API_KEY is required for the Responses engine".into()
            }
            Self::NoModelProviderAvailable => "no_model_provider_available".into(),
        }
    }
}

pub fn codex_availability_is_usable(codex: &CodexSubscriptionAvailability) -> bool {
    matches!(codex, CodexSubscriptionAvailability::Available { .. })
}

pub fn resolve_run_engine(
    mode: RunEngineMode,
    openai_api_key: Option<&str>,
    codex: &CodexSubscriptionAvailability,
) -> Result<SelectedRunEngine, ResolveRunEngineError> {
    let has_api_key = openai_api_key.is_some_and(|k| !k.is_empty());

    match mode {
        RunEngineMode::Codex => {
            if codex_availability_is_usable(codex) {
                Ok(SelectedRunEngine::CodexSubscription)
            } else {
                Err(ResolveRunEngineError::CodexUnavailable(
                    codex_unavailable_message(codex),
                ))
            }
        }
        RunEngineMode::Responses => {
            if has_api_key {
                Ok(SelectedRunEngine::ResponsesApi)
            } else {
                Err(ResolveRunEngineError::ResponsesApiKeyRequired)
            }
        }
        RunEngineMode::Auto => {
            if codex_availability_is_usable(codex) {
                Ok(SelectedRunEngine::CodexSubscription)
            } else {
                Err(ResolveRunEngineError::NoModelProviderAvailable)
            }
        }
    }
}

fn codex_unavailable_message(codex: &CodexSubscriptionAvailability) -> String {
    match codex {
        CodexSubscriptionAvailability::NotInstalled => "codex_not_installed".into(),
        CodexSubscriptionAvailability::NotAuthenticated => "codex_not_authenticated".into(),
        CodexSubscriptionAvailability::NotChatGpt => "codex_not_chatgpt".into(),
        CodexSubscriptionAvailability::Unavailable(msg) => msg.clone(),
        CodexSubscriptionAvailability::Available { .. } => "codex_available".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn available() -> CodexSubscriptionAvailability {
        CodexSubscriptionAvailability::Available {
            plan_type: Some("pro".into()),
        }
    }

    #[test]
    fn explicit_codex_available() {
        assert_eq!(
            resolve_run_engine(RunEngineMode::Codex, Some("sk-test"), &available()).unwrap(),
            SelectedRunEngine::CodexSubscription
        );
    }

    #[test]
    fn explicit_codex_unavailable_errors() {
        assert!(matches!(
            resolve_run_engine(
                RunEngineMode::Codex,
                Some("sk-test"),
                &CodexSubscriptionAvailability::NotAuthenticated
            ),
            Err(ResolveRunEngineError::CodexUnavailable(_))
        ));
    }

    #[test]
    fn explicit_responses_requires_key() {
        assert_eq!(
            resolve_run_engine(RunEngineMode::Responses, Some("sk-test"), &available()).unwrap(),
            SelectedRunEngine::ResponsesApi
        );
        assert_eq!(
            resolve_run_engine(RunEngineMode::Responses, None, &available()),
            Err(ResolveRunEngineError::ResponsesApiKeyRequired)
        );
    }

    #[test]
    fn auto_prefers_codex_when_available() {
        assert_eq!(
            resolve_run_engine(RunEngineMode::Auto, Some("sk-test"), &available()).unwrap(),
            SelectedRunEngine::CodexSubscription
        );
    }

    #[test]
    fn auto_never_falls_back_to_paid_api() {
        for unavailable in [
            CodexSubscriptionAvailability::NotInstalled,
            CodexSubscriptionAvailability::NotChatGpt,
            CodexSubscriptionAvailability::NotAuthenticated,
            CodexSubscriptionAvailability::Unavailable("offline".into()),
        ] {
            assert_eq!(
                resolve_run_engine(RunEngineMode::Auto, Some("test-key"), &unavailable),
                Err(ResolveRunEngineError::NoModelProviderAvailable)
            );
        }
    }

    #[test]
    fn auto_no_providers() {
        assert_eq!(
            resolve_run_engine(
                RunEngineMode::Auto,
                None,
                &CodexSubscriptionAvailability::NotInstalled
            ),
            Err(ResolveRunEngineError::NoModelProviderAvailable)
        );
    }

    #[test]
    fn invalid_env_value() {
        assert!(parse_run_engine_mode("codexx").is_err());
    }
}
