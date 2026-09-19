use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};

use crate::bot_context::{self, ContextInput};
use crate::db::resources::{get_bot_for_owner, patch_bot, BotRow};
use crate::error::ApiError;
use crate::AppState;

use super::generate::{generate_onboarding_turn, luna_generation_model};
use super::types::{
    BotOnboardingRecord, OnboardingAnswer, OnboardingDraft, OnboardingGenerateInput,
    OnboardingModelResponse, OnboardingQuestion, OnboardingStatus, MAX_LAST_ERROR_CHARS,
    MAX_ONBOARDING_QUESTIONS,
};
use super::validate::validate_onboarding_answer;

#[derive(Debug, Clone, sqlx::FromRow)]
struct OnboardingRow {
    bot_id: String,
    owner_id: String,
    status: String,
    questions_asked: i32,
    current_question: Option<Value>,
    answers: Value,
    draft_configuration: Option<Value>,
    last_error: Option<String>,
    revision: i64,
}

fn db(error: sqlx::Error) -> ApiError {
    ApiError::Internal(error.to_string())
}

fn parse_row(row: OnboardingRow) -> Result<BotOnboardingRecord, ApiError> {
    let status = OnboardingStatus::parse(&row.status)
        .ok_or_else(|| ApiError::Internal("invalid onboarding status".into()))?;
    let current_question = match row.current_question {
        Some(value) if !value.is_null() => Some(
            serde_json::from_value::<OnboardingQuestion>(value)
                .map_err(|e| ApiError::Internal(e.to_string()))?,
        ),
        _ => None,
    };
    let answers = serde_json::from_value::<Vec<OnboardingAnswer>>(row.answers)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let draft = match row.draft_configuration {
        Some(value) if !value.is_null() => Some(
            serde_json::from_value::<OnboardingDraft>(value)
                .map_err(|e| ApiError::Internal(e.to_string()))?,
        ),
        _ => None,
    };
    Ok(BotOnboardingRecord {
        bot_id: row.bot_id,
        owner_id: row.owner_id,
        status,
        questions_asked: row.questions_asked,
        current_question,
        answers,
        draft,
        last_error: row.last_error,
        revision: row.revision,
    })
}

async fn require_bot(pool: &PgPool, owner_id: &str, bot_id: &str) -> Result<BotRow, ApiError> {
    get_bot_for_owner(pool, owner_id, bot_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::NotFound)
}

async fn lock_owned_bot(
    tx: &mut Transaction<'_, Postgres>,
    owner_id: &str,
    bot_id: &str,
) -> Result<(), ApiError> {
    let exists: Option<String> =
        sqlx::query_scalar("SELECT id FROM bots WHERE id = $1 AND owner_id = $2 FOR UPDATE")
            .bind(bot_id)
            .bind(owner_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(db)?;
    exists.ok_or(ApiError::NotFound).map(|_| ())
}

async fn load_row(
    executor: impl sqlx::Executor<'_, Database = Postgres>,
    owner_id: &str,
    bot_id: &str,
    for_update: bool,
) -> Result<Option<BotOnboardingRecord>, ApiError> {
    let sql = if for_update {
        r#"
        SELECT bot_id, owner_id, status, questions_asked, current_question, answers,
               draft_configuration, last_error, revision
        FROM bot_onboarding
        WHERE bot_id = $1 AND owner_id = $2
        FOR UPDATE
        "#
    } else {
        r#"
        SELECT bot_id, owner_id, status, questions_asked, current_question, answers,
               draft_configuration, last_error, revision
        FROM bot_onboarding
        WHERE bot_id = $1 AND owner_id = $2
        "#
    };
    let row = sqlx::query_as::<_, OnboardingRow>(sql)
        .bind(bot_id)
        .bind(owner_id)
        .fetch_optional(executor)
        .await
        .map_err(db)?;
    row.map(parse_row).transpose()
}

async fn upsert(
    tx: &mut Transaction<'_, Postgres>,
    record: &BotOnboardingRecord,
) -> Result<BotOnboardingRecord, ApiError> {
    let current_question = record
        .current_question
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let answers =
        serde_json::to_value(&record.answers).map_err(|e| ApiError::Internal(e.to_string()))?;
    let draft = record
        .draft
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let last_error = record
        .last_error
        .as_deref()
        .map(|value| crate::bounded_text::truncate_utf8_bytes(value, MAX_LAST_ERROR_CHARS));
    let row = sqlx::query_as::<_, OnboardingRow>(
        r#"
        INSERT INTO bot_onboarding (
            bot_id, owner_id, status, questions_asked, current_question, answers,
            draft_configuration, last_error, revision, created_at, updated_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,NOW(),NOW())
        ON CONFLICT (bot_id) DO UPDATE SET
            status = EXCLUDED.status,
            questions_asked = EXCLUDED.questions_asked,
            current_question = EXCLUDED.current_question,
            answers = EXCLUDED.answers,
            draft_configuration = EXCLUDED.draft_configuration,
            last_error = EXCLUDED.last_error,
            revision = EXCLUDED.revision,
            updated_at = NOW()
        RETURNING bot_id, owner_id, status, questions_asked, current_question, answers,
                  draft_configuration, last_error, revision
        "#,
    )
    .bind(&record.bot_id)
    .bind(&record.owner_id)
    .bind(record.status.as_str())
    .bind(record.questions_asked)
    .bind(current_question)
    .bind(answers)
    .bind(draft)
    .bind(last_error)
    .bind(record.revision)
    .fetch_one(&mut **tx)
    .await
    .map_err(db)?;
    parse_row(row)
}

fn empty_record(bot_id: &str, owner_id: &str) -> BotOnboardingRecord {
    BotOnboardingRecord {
        bot_id: bot_id.to_string(),
        owner_id: owner_id.to_string(),
        status: OnboardingStatus::NotStarted,
        questions_asked: 0,
        current_question: None,
        answers: Vec::new(),
        draft: None,
        last_error: None,
        revision: 0,
    }
}

async fn generate_input(
    pool: &PgPool,
    bot: &BotRow,
    record: &BotOnboardingRecord,
) -> Result<OnboardingGenerateInput, ApiError> {
    let context = bot_context::get(pool, &bot.owner_id, &bot.id).await?;
    Ok(OnboardingGenerateInput {
        bot_name: bot.name.clone(),
        bot_instructions: bot.system_prompt.clone(),
        bot_model: bot.model.clone(),
        pinned_context: context.content,
        answers: record.answers.clone(),
        questions_asked: record.questions_asked,
        generation_model: luna_generation_model(),
    })
}

fn apply_model_response(record: &mut BotOnboardingRecord, response: OnboardingModelResponse) {
    record.last_error = None;
    match response {
        OnboardingModelResponse::Question { question } => {
            record.status = OnboardingStatus::InProgress;
            record.current_question = Some(question);
            record.draft = None;
        }
        OnboardingModelResponse::Complete { draft } => {
            record.status = OnboardingStatus::ReadyToApply;
            record.current_question = None;
            record.draft = Some(draft);
        }
    }
}

async fn run_generation(
    state: &AppState,
    bot: &BotRow,
    record: &mut BotOnboardingRecord,
) -> Result<(), ApiError> {
    let input = generate_input(&state.pool, bot, record).await?;
    match generate_onboarding_turn(state, &bot.owner_id, input).await {
        Ok(response) => {
            apply_model_response(record, response);
            Ok(())
        }
        Err(err) => {
            record.last_error = Some(err.to_string());
            Err(err)
        }
    }
}

fn bump_revision(record: &mut BotOnboardingRecord) {
    record.revision = record.revision.saturating_add(1);
}

fn answer_matches(
    existing: &OnboardingAnswer,
    option_id: Option<&str>,
    custom_text: Option<&str>,
) -> bool {
    let option = option_id.map(str::trim).filter(|value| !value.is_empty());
    let custom = custom_text.map(str::trim).filter(|value| !value.is_empty());
    match (
        existing.option_id.as_deref(),
        existing.custom_text.as_deref(),
    ) {
        (Some(id), None) => option == Some(id) && custom.is_none(),
        (None, Some(text)) => option.is_none() && custom == Some(text),
        _ => false,
    }
}

async fn persist(
    pool: &PgPool,
    record: &BotOnboardingRecord,
) -> Result<BotOnboardingRecord, ApiError> {
    let mut tx = pool.begin().await.map_err(db)?;
    let saved = upsert(&mut tx, record).await?;
    tx.commit().await.map_err(db)?;
    Ok(saved)
}

pub async fn get_state(
    pool: &PgPool,
    owner_id: &str,
    bot_id: &str,
) -> Result<BotOnboardingRecord, ApiError> {
    let bot = require_bot(pool, owner_id, bot_id).await?;
    if let Some(record) = load_row(pool, owner_id, bot_id, false).await? {
        return Ok(record);
    }
    Ok(empty_record(&bot.id, owner_id))
}

pub async fn start_setup(
    state: &AppState,
    owner_id: &str,
    bot_id: &str,
    restart: bool,
) -> Result<BotOnboardingRecord, ApiError> {
    let bot = require_bot(&state.pool, owner_id, bot_id).await?;
    let mut tx = state.pool.begin().await.map_err(db)?;
    lock_owned_bot(&mut tx, owner_id, bot_id).await?;
    let mut record = load_row(&mut *tx, owner_id, bot_id, true)
        .await?
        .unwrap_or_else(|| empty_record(&bot.id, owner_id));

    let should_generate = if restart
        || matches!(
            record.status,
            OnboardingStatus::NotStarted
                | OnboardingStatus::Completed
                | OnboardingStatus::Dismissed
        ) {
        record.status = OnboardingStatus::InProgress;
        record.questions_asked = 0;
        record.current_question = None;
        record.answers.clear();
        record.draft = None;
        record.last_error = None;
        bump_revision(&mut record);
        true
    } else if record.status == OnboardingStatus::ReadyToApply && record.draft.is_some() {
        false
    } else if record.current_question.is_some() && record.last_error.is_none() {
        false
    } else {
        record.status = OnboardingStatus::InProgress;
        bump_revision(&mut record);
        true
    };

    if !should_generate {
        tx.commit().await.map_err(db)?;
        return Ok(record);
    }

    let saved = upsert(&mut tx, &record).await?;
    tx.commit().await.map_err(db)?;
    record = saved;

    let generate_result = run_generation(state, &bot, &mut record).await;
    bump_revision(&mut record);
    let saved = persist(&state.pool, &record).await?;
    generate_result?;
    Ok(saved)
}

pub async fn answer_setup(
    state: &AppState,
    owner_id: &str,
    bot_id: &str,
    question_id: &str,
    option_id: Option<&str>,
    custom_text: Option<&str>,
    expected_revision: Option<i64>,
) -> Result<BotOnboardingRecord, ApiError> {
    let bot = require_bot(&state.pool, owner_id, bot_id).await?;
    let mut tx = state.pool.begin().await.map_err(db)?;
    lock_owned_bot(&mut tx, owner_id, bot_id).await?;
    let mut record = load_row(&mut *tx, owner_id, bot_id, true)
        .await?
        .ok_or_else(|| ApiError::Conflict("Start setup before answering".into()))?;

    if let Some(last) = record
        .answers
        .last()
        .filter(|answer| answer.question_id == question_id)
    {
        if answer_matches(last, option_id, custom_text) {
            tx.commit().await.map_err(db)?;
            return Ok(record);
        }
        return Err(ApiError::Conflict(
            "That setup question was already answered".into(),
        ));
    }

    if let Some(expected) = expected_revision {
        if expected != record.revision {
            return Err(ApiError::Conflict(
                "This setup question changed. Reload it before answering.".into(),
            ));
        }
    }

    let current = record
        .current_question
        .clone()
        .ok_or_else(|| ApiError::Conflict("There is no open setup question to answer".into()))?;
    if current.id != question_id {
        return Err(ApiError::Conflict(
            "That question is no longer the current setup step".into(),
        ));
    }
    if record.questions_asked >= MAX_ONBOARDING_QUESTIONS {
        return Err(ApiError::Validation(
            "setup already asked the maximum number of questions".into(),
        ));
    }

    let answer = validate_onboarding_answer(&current, option_id, custom_text)?;
    record.answers.push(answer);
    record.questions_asked += 1;
    record.current_question = None;
    record.last_error = None;
    record.status = OnboardingStatus::InProgress;
    bump_revision(&mut record);
    let saved = upsert(&mut tx, &record).await?;
    tx.commit().await.map_err(db)?;
    record = saved;

    let generate_result = run_generation(state, &bot, &mut record).await;
    bump_revision(&mut record);
    let saved = persist(&state.pool, &record).await?;
    generate_result?;
    Ok(saved)
}

pub async fn apply_setup(
    state: &AppState,
    owner_id: &str,
    bot_id: &str,
    expected_revision: Option<i64>,
) -> Result<BotOnboardingRecord, ApiError> {
    let mut tx = state.pool.begin().await.map_err(db)?;
    lock_owned_bot(&mut tx, owner_id, bot_id).await?;
    let mut record = load_row(&mut *tx, owner_id, bot_id, true)
        .await?
        .ok_or_else(|| ApiError::Conflict("Start setup before applying it".into()))?;

    if record.status == OnboardingStatus::Completed {
        tx.commit().await.map_err(db)?;
        return Ok(record);
    }
    if record.status != OnboardingStatus::ReadyToApply {
        return Err(ApiError::Conflict(
            "Finish the setup questions before applying".into(),
        ));
    }
    if let Some(expected) = expected_revision {
        if expected != record.revision {
            return Err(ApiError::Conflict(
                "This setup changed in another window. Reload it before applying.".into(),
            ));
        }
    }
    let draft = record
        .draft
        .clone()
        .ok_or_else(|| ApiError::Conflict("Setup has no configuration to apply".into()))?;
    tx.commit().await.map_err(db)?;

    patch_bot(
        &state.pool,
        owner_id,
        bot_id,
        None,
        Some(draft.instructions.as_str()),
        None,
        None,
        None,
        None,
        None,
    )
    .await?
    .ok_or(ApiError::NotFound)?;
    let existing = bot_context::get(&state.pool, owner_id, bot_id).await?;
    bot_context::save(
        &state.pool,
        owner_id,
        bot_id,
        ContextInput {
            content: draft.context.clone(),
            revision: existing.revision,
        },
    )
    .await?;

    record.status = OnboardingStatus::Completed;
    record.current_question = None;
    record.last_error = None;
    bump_revision(&mut record);
    persist(&state.pool, &record).await
}

pub async fn dismiss_setup(
    pool: &PgPool,
    owner_id: &str,
    bot_id: &str,
    expected_revision: Option<i64>,
) -> Result<BotOnboardingRecord, ApiError> {
    let mut tx = pool.begin().await.map_err(db)?;
    lock_owned_bot(&mut tx, owner_id, bot_id).await?;
    let mut record = load_row(&mut *tx, owner_id, bot_id, true)
        .await?
        .unwrap_or_else(|| empty_record(bot_id, owner_id));
    if record.status == OnboardingStatus::Dismissed || record.status == OnboardingStatus::Completed
    {
        tx.commit().await.map_err(db)?;
        return Ok(record);
    }
    if let Some(expected) = expected_revision {
        if expected != record.revision && record.revision != 0 {
            return Err(ApiError::Conflict(
                "This setup changed in another window. Reload it before skipping.".into(),
            ));
        }
    }
    record.status = OnboardingStatus::Dismissed;
    record.current_question = None;
    record.last_error = None;
    bump_revision(&mut record);
    let saved = upsert(&mut tx, &record).await?;
    tx.commit().await.map_err(db)?;
    Ok(saved)
}
