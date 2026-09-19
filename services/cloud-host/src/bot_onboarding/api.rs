use axum::extract::{Path, State};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::auth::Principal;
use crate::error::ApiError;

use super::service;
use super::types::{
    BotOnboardingRecord, OnboardingAnswer, OnboardingDraft, OnboardingQuestion,
    MAX_ONBOARDING_QUESTIONS,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingQuestionView {
    pub id: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub helper: Option<String>,
    pub options: Vec<OnboardingOptionView>,
    pub allow_custom: bool,
    pub index: i32,
    pub max_questions: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingOptionView {
    pub id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingDraftView {
    pub summary: String,
    pub owns: String,
    pub working_style: String,
    pub instructions: String,
    pub context: String,
    pub suggested_first_tasks: Vec<String>,
    pub suggested_capabilities: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingResponse {
    pub bot_id: String,
    pub status: String,
    pub questions_asked: i32,
    pub max_questions: i32,
    pub revision: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_question: Option<OnboardingQuestionView>,
    pub answers: Vec<OnboardingAnswer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft: Option<OnboardingDraftView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    pub generation_model: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartOnboardingRequest {
    #[serde(default)]
    pub restart: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnswerOnboardingRequest {
    pub question_id: String,
    pub option_id: Option<String>,
    pub custom_text: Option<String>,
    pub revision: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOnboardingRequest {
    pub revision: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DismissOnboardingRequest {
    pub revision: Option<i64>,
}

fn question_view(question: OnboardingQuestion, index: i32) -> OnboardingQuestionView {
    OnboardingQuestionView {
        id: question.id,
        prompt: question.prompt,
        helper: question.helper,
        options: question
            .options
            .into_iter()
            .map(|option| OnboardingOptionView {
                id: option.id,
                label: option.label,
                description: option.description,
            })
            .collect(),
        allow_custom: question.allow_custom,
        index,
        max_questions: MAX_ONBOARDING_QUESTIONS,
    }
}

fn draft_view(draft: OnboardingDraft) -> OnboardingDraftView {
    OnboardingDraftView {
        summary: draft.summary,
        owns: draft.owns,
        working_style: draft.working_style,
        instructions: draft.instructions,
        context: draft.context,
        suggested_first_tasks: draft.suggested_first_tasks,
        suggested_capabilities: draft.suggested_capabilities,
    }
}

fn to_response(record: BotOnboardingRecord) -> OnboardingResponse {
    let question_index = if record.current_question.is_some() {
        record.questions_asked + 1
    } else {
        record.questions_asked
    };
    OnboardingResponse {
        bot_id: record.bot_id,
        status: record.status.as_str().to_string(),
        questions_asked: record.questions_asked,
        max_questions: MAX_ONBOARDING_QUESTIONS,
        revision: record.revision,
        current_question: record
            .current_question
            .map(|question| question_view(question, question_index)),
        answers: record.answers,
        draft: record.draft.map(draft_view),
        last_error: record.last_error,
        generation_model: super::generate::luna_generation_model(),
    }
}

pub async fn get(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(bot_id): Path<String>,
) -> Result<Json<OnboardingResponse>, ApiError> {
    let record = service::get_state(&state.pool, principal.owner_id(), &bot_id).await?;
    Ok(Json(to_response(record)))
}

pub async fn start(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(bot_id): Path<String>,
    Json(body): Json<StartOnboardingRequest>,
) -> Result<Json<OnboardingResponse>, ApiError> {
    let record =
        service::start_setup(&state, principal.owner_id(), &bot_id, body.restart).await?;
    Ok(Json(to_response(record)))
}

pub async fn answer(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(bot_id): Path<String>,
    Json(body): Json<AnswerOnboardingRequest>,
) -> Result<Json<OnboardingResponse>, ApiError> {
    let record = service::answer_setup(
        &state,
        principal.owner_id(),
        &bot_id,
        &body.question_id,
        body.option_id.as_deref(),
        body.custom_text.as_deref(),
        body.revision,
    )
    .await?;
    Ok(Json(to_response(record)))
}

pub async fn apply(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(bot_id): Path<String>,
    Json(body): Json<ApplyOnboardingRequest>,
) -> Result<Json<OnboardingResponse>, ApiError> {
    let record =
        service::apply_setup(&state, principal.owner_id(), &bot_id, body.revision).await?;
    Ok(Json(to_response(record)))
}

pub async fn dismiss(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(bot_id): Path<String>,
    Json(body): Json<DismissOnboardingRequest>,
) -> Result<Json<OnboardingResponse>, ApiError> {
    let record =
        service::dismiss_setup(&state.pool, principal.owner_id(), &bot_id, body.revision).await?;
    Ok(Json(to_response(record)))
}
