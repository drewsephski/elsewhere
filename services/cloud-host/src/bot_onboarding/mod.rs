//! Optional post-create Bot setup. Independent of agent runs and ask_user.

mod api;
mod generate;
mod service;
mod types;
mod validate;

pub use api::{answer, apply, dismiss, get, start};
pub use types::{
    OnboardingAnswer, OnboardingDraft, OnboardingGenerateInput, OnboardingModelResponse,
    OnboardingOption, OnboardingQuestion, OnboardingStatus, MAX_ONBOARDING_QUESTIONS,
};
pub use validate::{parse_onboarding_model_json, validate_onboarding_answer};

#[cfg(any(test, feature = "test-utils"))]
pub use generate::ONBOARDING_DEVELOPER_INSTRUCTIONS;
