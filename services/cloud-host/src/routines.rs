//! Recurring bot assignments: timezone-aware schedules, destinations, durable history.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

use crate::error::ApiError;
use crate::routine_runs::{self, RoutineRun};
use crate::routine_webhooks::{self, WebhookTriggerView};
use crate::schedule::{
    human_schedule_label, initial_next_run, next_after, parse_schedule, resume_next_run,
    ScheduleDefinition, ScheduleKind,
};
use chrono_tz::Tz;

pub use crate::schedule::next_occurrence;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Routine {
    pub id: String,
    #[serde(skip)]
    pub owner_id: String,
    pub bot_id: String,
    pub name: String,
    pub instructions: String,
    pub interval_minutes: i32,
    pub enabled: bool,
    pub next_run_at: DateTime<Utc>,
    pub last_run_id: Option<String>,
    pub last_error: Option<String>,
    #[serde(skip)]
    pub acknowledged_run_id: Option<String>,
    pub schedule_kind: String,
    pub schedule_expression: String,
    pub timezone: String,
    pub destination_conversation_id: Option<String>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_failure_at: Option<DateTime<Utc>>,
    pub consecutive_failures: i32,
    pub failure_policy: String,
    pub skill_id: Option<String>,
    pub pinned_skill_version: Option<i32>,
    pub trigger_mode: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutineView {
    pub id: String,
    pub bot_id: String,
    pub name: String,
    pub instructions: String,
    pub interval_minutes: i32,
    pub enabled: bool,
    pub next_run_at: DateTime<Utc>,
    pub last_run_id: Option<String>,
    pub last_error: Option<String>,
    pub schedule_kind: String,
    pub schedule_expression: String,
    pub timezone: String,
    pub schedule_label: String,
    pub destination_conversation_id: Option<String>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_failure_at: Option<DateTime<Utc>>,
    pub consecutive_failures: i32,
    pub failure_policy: String,
    pub skill_id: Option<String>,
    pub pinned_skill_version: Option<i32>,
    pub trigger_mode: String,
    pub webhook: WebhookTriggerView,
    pub recent_runs: Vec<RoutineRun>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoutineInput {
    pub bot_id: String,
    pub name: String,
    pub instructions: String,
    pub interval_minutes: Option<i32>,
    pub next_run_at: DateTime<Utc>,
    pub enabled: bool,
    pub schedule_kind: Option<String>,
    pub schedule_expression: Option<String>,
    pub timezone: Option<String>,
    pub destination_conversation_id: Option<String>,
    pub failure_policy: Option<String>,
    pub skill_id: Option<String>,
    pub pinned_skill_version: Option<i32>,
    pub trigger_mode: Option<String>,
}

impl RoutineInput {
    pub fn trigger_mode(&self) -> Result<&str, ApiError> {
        match self.trigger_mode.as_deref().unwrap_or("schedule") {
            mode @ ("schedule" | "webhook") => Ok(mode),
            _ => Err(ApiError::Validation("Unknown trigger".into())),
        }
    }

    pub fn schedule_definition(&self) -> Result<ScheduleDefinition, ApiError> {
        let kind = self.schedule_kind.as_deref().unwrap_or("interval");
        let expression = self
            .schedule_expression
            .clone()
            .unwrap_or_else(|| self.interval_minutes.unwrap_or(60).to_string());
        let timezone = self.timezone.as_deref().unwrap_or("UTC");
        parse_schedule(kind, &expression, timezone, self.interval_minutes)
    }

    pub fn validate(&self) -> Result<(), ApiError> {
        if self.name.trim().is_empty() || self.name.chars().count() > 100 {
            return Err(ApiError::Validation(
                "Give your routine a name of up to 100 characters".into(),
            ));
        }
        if self.instructions.trim().is_empty() || self.instructions.len() > 100_000 {
            return Err(ApiError::Validation(
                "Add an assignment of up to 100,000 bytes".into(),
            ));
        }
        let trigger_mode = self.trigger_mode()?;
        if trigger_mode == "schedule" {
            let _schedule = self.schedule_definition()?;
            let now = Utc::now();
            if self.next_run_at > now + chrono::Duration::days(366) {
                return Err(ApiError::Validation(
                    "Choose a first run within the next year".into(),
                ));
            }
        } else if self.schedule_kind.is_some() || self.schedule_expression.is_some() {
            let _schedule = self.schedule_definition()?;
        }
        if let Some(policy) = &self.failure_policy {
            if !matches!(policy.as_str(), "pause_after_failure" | "continue") {
                return Err(ApiError::Validation("Unknown failure policy".into()));
            }
        }
        if self.skill_id.is_none() && self.pinned_skill_version.is_some() {
            return Err(ApiError::Validation(
                "Choose a skill before pinning a version".into(),
            ));
        }
        Ok(())
    }
}

fn db_error(error: sqlx::Error) -> ApiError {
    ApiError::Internal(error.to_string())
}

fn interval_minutes_for_row(schedule: &ScheduleDefinition) -> i32 {
    if schedule.kind == ScheduleKind::Interval {
        schedule.expression.parse().unwrap_or(60)
    } else {
        60
    }
}

pub fn compose_routine_runtime_message(
    routine_name: &str,
    instructions: &str,
    scheduled_for: DateTime<Utc>,
    timezone: &str,
    destination_label: &str,
) -> String {
    let tz: Tz = timezone.parse().unwrap_or(chrono_tz::UTC);
    let local = scheduled_for.with_timezone(&tz);
    let occurrence = local.format("%Y-%m-%d %H:%M %Z").to_string();
    format!(
        "Scheduled routine: {routine_name}\n\nRoutine instructions:\n{instructions}\n\nScheduled occurrence:\n{occurrence} ({timezone})\n\nDestination:\n{destination_label}\n\nThis is unattended scheduled work.\nFollow normal approval boundaries.\nIf a required source is unavailable, report the failure or missing source rather than inventing stale information."
    )
}

async fn destination_label_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &str,
    bot_id: &str,
    destination_conversation_id: Option<&str>,
) -> Result<(String, String, bool), ApiError> {
    let conversation_id = if let Some(id) = destination_conversation_id {
        id.to_string()
    } else {
        crate::conversation::get_or_create_primary_conversation_id_in_tx(tx, owner, bot_id).await?
    };
    let row = sqlx::query(
        "SELECT conversation_type, name, bot_id FROM conversations WHERE id = $1 AND owner_id = $2",
    )
    .bind(&conversation_id)
    .bind(owner)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    .ok_or(ApiError::NotFound)?;
    let conversation_type: String = row.get("conversation_type");
    let label = if conversation_type == "group" {
        row.get::<Option<String>, _>("name")
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "Group conversation".into())
    } else {
        "Bot direct conversation".into()
    };
    Ok((conversation_id, label, conversation_type == "group"))
}

async fn validate_destination(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &str,
    bot_id: &str,
    destination_conversation_id: Option<&str>,
) -> Result<(), ApiError> {
    if let Some(id) = destination_conversation_id {
        crate::groups::assert_bot_may_use_conversation(tx, owner, bot_id, id).await?;
    }
    Ok(())
}

pub async fn list(pool: &PgPool, owner: &str) -> Result<Vec<RoutineView>, ApiError> {
    let rows: Vec<Routine> = sqlx::query_as(
        "SELECT * FROM routines WHERE owner_id = $1 ORDER BY created_at DESC LIMIT 100",
    )
    .bind(owner)
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    let routine_ids: Vec<String> = rows.iter().map(|row| row.id.clone()).collect();
    let recent_by_routine =
        routine_runs::list_recent_for_routines(pool, owner, &routine_ids, 20).await?;
    let webhook_by_routine =
        routine_webhooks::metadata_for_routines(pool, owner, &routine_ids).await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let recent_runs = recent_by_routine.get(&row.id).cloned().unwrap_or_default();
        let webhook = webhook_by_routine
            .get(&row.id)
            .cloned()
            .unwrap_or_else(WebhookTriggerView::absent);
        out.push(to_view_with_recent_runs(row, recent_runs, webhook)?);
    }
    Ok(out)
}

pub async fn get(pool: &PgPool, owner: &str, id: &str) -> Result<RoutineView, ApiError> {
    let row: Routine = sqlx::query_as("SELECT * FROM routines WHERE id = $1 AND owner_id = $2")
        .bind(id)
        .bind(owner)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?
        .ok_or(ApiError::NotFound)?;
    to_view(pool, row).await
}

async fn to_view(pool: &PgPool, row: Routine) -> Result<RoutineView, ApiError> {
    to_view_with_secret(pool, row, None).await
}

async fn to_view_with_secret(
    pool: &PgPool,
    row: Routine,
    revealed_webhook_url: Option<String>,
) -> Result<RoutineView, ApiError> {
    let recent_runs = routine_runs::list_for_routine(pool, &row.owner_id, &row.id, 20).await?;
    let mut webhook = routine_webhooks::get_for_owner(pool, &row.owner_id, &row.id).await?;
    if let Some(url) = revealed_webhook_url {
        webhook = webhook.with_revealed_url(url);
    }
    to_view_with_recent_runs(row, recent_runs, webhook)
}

fn schedule_label_for(row: &Routine, schedule: &ScheduleDefinition) -> String {
    if row.trigger_mode == "webhook" {
        "Webhook".into()
    } else {
        format!("Scheduled · {}", human_schedule_label(schedule))
    }
}

fn to_view_with_recent_runs(
    row: Routine,
    recent_runs: Vec<RoutineRun>,
    webhook: WebhookTriggerView,
) -> Result<RoutineView, ApiError> {
    let schedule = parse_schedule(
        &row.schedule_kind,
        &row.schedule_expression,
        &row.timezone,
        Some(row.interval_minutes),
    )?;
    let schedule_label = schedule_label_for(&row, &schedule);
    Ok(RoutineView {
        id: row.id,
        bot_id: row.bot_id,
        name: row.name,
        instructions: row.instructions,
        interval_minutes: row.interval_minutes,
        enabled: row.enabled,
        next_run_at: row.next_run_at,
        last_run_id: row.last_run_id,
        last_error: row.last_error,
        schedule_kind: row.schedule_kind,
        schedule_expression: row.schedule_expression,
        timezone: row.timezone,
        schedule_label,
        destination_conversation_id: row.destination_conversation_id,
        last_success_at: row.last_success_at,
        last_failure_at: row.last_failure_at,
        consecutive_failures: row.consecutive_failures,
        failure_policy: row.failure_policy,
        skill_id: row.skill_id,
        pinned_skill_version: row.pinned_skill_version,
        trigger_mode: row.trigger_mode,
        webhook,
        recent_runs,
    })
}

pub async fn save(
    pool: &PgPool,
    owner: &str,
    id: Option<&str>,
    input: &RoutineInput,
) -> Result<RoutineView, ApiError> {
    save_with_origin(pool, owner, id, input, None).await
}

pub async fn save_with_origin(
    pool: &PgPool,
    owner: &str,
    id: Option<&str>,
    input: &RoutineInput,
    public_origin: Option<&str>,
) -> Result<RoutineView, ApiError> {
    input.validate()?;
    let trigger_mode = input.trigger_mode()?;
    let schedule = if trigger_mode == "webhook" {
        input
            .schedule_definition()
            .unwrap_or_else(|_| default_webhook_placeholder_schedule())
    } else {
        input.schedule_definition()?
    };
    let interval_minutes = interval_minutes_for_row(&schedule);
    let failure_policy = input
        .failure_policy
        .as_deref()
        .unwrap_or("pause_after_failure");
    let now = Utc::now();

    let mut tx = pool.begin().await.map_err(db_error)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(format!("routines-owner:{owner}"))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

    let existing: Option<Routine> = if let Some(id) = id {
        sqlx::query_as("SELECT * FROM routines WHERE id = $1 AND owner_id = $2 FOR UPDATE")
            .bind(id)
            .bind(owner)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_error)?
    } else {
        None
    };
    if id.is_some() && existing.is_none() {
        return Err(ApiError::NotFound);
    }
    if existing.is_none() {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM routines WHERE owner_id=$1")
            .bind(owner)
            .fetch_one(&mut *tx)
            .await
            .map_err(db_error)?;
        if count >= 100 {
            return Err(ApiError::Validation(
                "You can keep up to 100 routines".into(),
            ));
        }
    }

    let next_run_at = if trigger_mode == "webhook" {
        existing
            .as_ref()
            .map(|row| row.next_run_at)
            .unwrap_or(now + chrono::Duration::days(365 * 40))
    } else {
        initial_next_run(&schedule, input.next_run_at, now)?
    };

    let bot_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM routines WHERE owner_id = $1 AND bot_id = $2 AND ($3::text IS NULL OR id <> $3)",
    )
    .bind(owner)
    .bind(&input.bot_id)
    .bind(id)
    .fetch_one(&mut *tx)
    .await
    .map_err(db_error)?;
    if bot_count >= 50 {
        return Err(ApiError::Validation(
            "Each bot can have up to 50 routines".into(),
        ));
    }

    let allowed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM bots b JOIN sandboxes s ON s.id = b.computer_id AND s.owner_id = b.owner_id WHERE b.id=$1 AND b.owner_id=$2 AND s.state <> 'archived')",
    )
    .bind(&input.bot_id)
    .bind(owner)
    .fetch_one(&mut *tx)
    .await
    .map_err(db_error)?;
    if !allowed {
        return Err(ApiError::Validation(
            "Choose one of your bots with an available computer".into(),
        ));
    }

    validate_destination(
        &mut tx,
        owner,
        &input.bot_id,
        input.destination_conversation_id.as_deref(),
    )
    .await?;

    if let Some(skill_id) = input.skill_id.as_deref() {
        let skill = crate::skills::get_skill_for_owner(pool, owner, skill_id)
            .await?
            .ok_or(ApiError::Validation("Choose one of your skills".into()))?;
        if skill.status != "active" {
            return Err(ApiError::Validation(
                "Archived skills cannot be used in routines".into(),
            ));
        }
        if let Some(version) = input.pinned_skill_version {
            if crate::skills::get_version_for_owner(pool, owner, skill_id, version)
                .await?
                .is_none()
            {
                return Err(ApiError::Validation(
                    "Pinned skill version not found".into(),
                ));
            }
        }
    }

    let new_id = id
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let row: Routine = sqlx::query_as(
        r#"
        INSERT INTO routines (
            id, owner_id, bot_id, name, instructions, interval_minutes, next_run_at, enabled,
            schedule_kind, schedule_expression, timezone, destination_conversation_id, failure_policy,
            skill_id, pinned_skill_version, trigger_mode
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)
        ON CONFLICT (id) DO UPDATE SET
            bot_id = EXCLUDED.bot_id,
            name = EXCLUDED.name,
            instructions = EXCLUDED.instructions,
            interval_minutes = EXCLUDED.interval_minutes,
            next_run_at = EXCLUDED.next_run_at,
            enabled = EXCLUDED.enabled,
            schedule_kind = EXCLUDED.schedule_kind,
            schedule_expression = EXCLUDED.schedule_expression,
            timezone = EXCLUDED.timezone,
            destination_conversation_id = EXCLUDED.destination_conversation_id,
            failure_policy = EXCLUDED.failure_policy,
            skill_id = EXCLUDED.skill_id,
            pinned_skill_version = EXCLUDED.pinned_skill_version,
            trigger_mode = EXCLUDED.trigger_mode,
            acknowledged_run_id = CASE WHEN EXCLUDED.enabled THEN routines.last_run_id ELSE routines.acknowledged_run_id END,
            last_error = NULL,
            updated_at = NOW()
        WHERE routines.owner_id = EXCLUDED.owner_id
        RETURNING *
        "#,
    )
    .bind(&new_id)
    .bind(owner)
    .bind(&input.bot_id)
    .bind(input.name.trim())
    .bind(input.instructions.trim())
    .bind(interval_minutes)
    .bind(next_run_at)
    .bind(input.enabled)
    .bind(schedule.kind.as_str())
    .bind(&schedule.expression)
    .bind(schedule.timezone.to_string())
    .bind(input.destination_conversation_id.as_deref())
    .bind(failure_policy)
    .bind(input.skill_id.as_deref())
    .bind(input.pinned_skill_version)
    .bind(trigger_mode)
    .fetch_one(&mut *tx)
    .await
    .map_err(db_error)?;

    let revealed_url = if trigger_mode == "webhook" {
        routine_webhooks::ensure_in_tx(&mut tx, owner, &row.id)
            .await?
            .map(|token| routine_webhooks::public_webhook_url(public_origin, &token))
    } else {
        routine_webhooks::delete_in_tx(&mut tx, owner, &row.id).await?;
        None
    };

    tx.commit().await.map_err(db_error)?;
    to_view_with_secret(pool, row, revealed_url).await
}

fn default_webhook_placeholder_schedule() -> ScheduleDefinition {
    parse_schedule("interval", "60", "UTC", Some(60))
        .expect("placeholder interval schedule is valid")
}

pub async fn delete(pool: &PgPool, owner: &str, id: &str) -> Result<(), ApiError> {
    let result = sqlx::query("DELETE FROM routines WHERE id = $1 AND owner_id = $2")
        .bind(id)
        .bind(owner)
        .execute(pool)
        .await
        .map_err(db_error)?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(())
}

pub async fn set_enabled(
    pool: &PgPool,
    owner: &str,
    id: &str,
    enabled: bool,
) -> Result<RoutineView, ApiError> {
    let mut tx = pool.begin().await.map_err(db_error)?;
    let row: Routine =
        sqlx::query_as("SELECT * FROM routines WHERE id = $1 AND owner_id = $2 FOR UPDATE")
            .bind(id)
            .bind(owner)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_error)?
            .ok_or(ApiError::NotFound)?;

    let next_run_at = if enabled && row.trigger_mode != "webhook" {
        let now = Utc::now();
        let schedule = parse_schedule(
            &row.schedule_kind,
            &row.schedule_expression,
            &row.timezone,
            Some(row.interval_minutes),
        )?;
        resume_next_run(&schedule, row.next_run_at, now)?
    } else {
        row.next_run_at
    };

    let row: Routine = sqlx::query_as(
        r#"
        UPDATE routines SET
            enabled = $3,
            acknowledged_run_id = CASE WHEN $3 THEN last_run_id ELSE acknowledged_run_id END,
            next_run_at = $4,
            last_error = CASE WHEN $3 THEN NULL ELSE last_error END,
            updated_at = NOW()
        WHERE id = $1 AND owner_id = $2
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(owner)
    .bind(enabled)
    .bind(next_run_at)
    .fetch_one(&mut *tx)
    .await
    .map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    to_view(pool, row).await
}

async fn admit_routine_work(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    routine: &Routine,
    scheduled_for: DateTime<Utc>,
    trigger_kind: &str,
    idempotency_key: Option<&str>,
    advance_schedule: bool,
    now: DateTime<Utc>,
    runtime_message_override: Option<String>,
) -> Result<Option<String>, ApiError> {
    let schedule = parse_schedule(
        &routine.schedule_kind,
        &routine.schedule_expression,
        &routine.timezone,
        Some(routine.interval_minutes),
    )?;
    let occurrence_id = routine_runs::admit_occurrence_in_tx(
        tx,
        &routine.owner_id,
        &routine.id,
        scheduled_for,
        trigger_kind,
        idempotency_key,
    )
    .await?;

    let existing_run: Option<String> =
        sqlx::query_scalar::<_, Option<String>>("SELECT run_id FROM routine_runs WHERE id = $1")
            .bind(&occurrence_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(db_error)?
            .flatten();
    if let Some(run_id) = existing_run {
        return Ok(Some(run_id));
    }

    let (conversation_id, destination_label, is_group) = destination_label_in_tx(
        tx,
        &routine.owner_id,
        &routine.bot_id,
        routine.destination_conversation_id.as_deref(),
    )
    .await?;

    let request_key = if trigger_kind == "scheduled" {
        format!(
            "routine:{}:{}",
            routine.id,
            scheduled_for.timestamp_millis()
        )
    } else if let Some(key) = idempotency_key {
        format!("routine-{trigger_kind}:{id}:{key}", id = routine.id)
    } else {
        format!(
            "routine-{trigger_kind}:{id}:{occurrence}",
            id = routine.id,
            occurrence = occurrence_id
        )
    };

    let runtime_message = runtime_message_override.unwrap_or_else(|| {
        compose_routine_runtime_message(
            &routine.name,
            &routine.instructions,
            scheduled_for,
            &routine.timezone,
            &destination_label,
        )
    });

    sqlx::query("SAVEPOINT routine_admission")
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
    let work = match crate::work::enqueue_routine_in_transaction(
        tx,
        &routine.owner_id,
        &request_key,
        &routine.bot_id,
        &conversation_id,
        &routine.id,
        &routine.name,
        &runtime_message,
        is_group,
    )
    .await
    {
        Ok(work) => work,
        Err(err) => {
            sqlx::query("ROLLBACK TO SAVEPOINT routine_admission")
                .execute(&mut **tx)
                .await
                .map_err(db_error)?;
            return Err(err);
        }
    };

    routine_runs::link_run_in_tx(tx, &occurrence_id, &work.run_id).await?;

    let next = if advance_schedule {
        next_after(&schedule, scheduled_for, now)?
    } else {
        routine.next_run_at
    };

    sqlx::query(
        r#"
        UPDATE routines SET last_run_id = $2, next_run_at = $3, last_error = NULL, updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(&routine.id)
    .bind(&work.run_id)
    .bind(next)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;

    Ok(Some(work.run_id))
}

pub async fn tick(pool: &PgPool, now: DateTime<Utc>) -> Result<usize, ApiError> {
    let mut count = 0;
    for _ in 0..20 {
        let mut tx = pool.begin().await.map_err(db_error)?;
        let routine: Option<Routine> = sqlx::query_as(
            "SELECT * FROM routines WHERE enabled AND trigger_mode = 'schedule' AND next_run_at <= $1 ORDER BY next_run_at LIMIT 1 FOR UPDATE SKIP LOCKED",
        )
        .bind(now)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;
        let Some(routine) = routine else {
            break;
        };

        let schedule = parse_schedule(
            &routine.schedule_kind,
            &routine.schedule_expression,
            &routine.timezone,
            Some(routine.interval_minutes),
        )?;
        let scheduled_for = routine.next_run_at;
        let next = next_after(&schedule, scheduled_for, now)?;

        let previous: Option<String> = if let Some(id) = &routine.last_run_id {
            sqlx::query_scalar("SELECT status FROM agent_runs WHERE id=$1")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(db_error)?
        } else {
            None
        };

        if previous
            .as_deref()
            .is_some_and(|s| matches!(s, "queued" | "running"))
        {
            routine_runs::record_skipped_scheduled_in_tx(
                &mut tx,
                &routine.owner_id,
                &routine.id,
                scheduled_for,
                "previous_run_active",
                "Skipped — previous run still in progress",
            )
            .await?;
            sqlx::query("UPDATE routines SET next_run_at=$2, updated_at=NOW() WHERE id=$1")
                .bind(&routine.id)
                .bind(next)
                .execute(&mut *tx)
                .await
                .map_err(db_error)?;
        } else if routine.failure_policy == "pause_after_failure"
            && routine.acknowledged_run_id != routine.last_run_id
            && previous
                .as_deref()
                .is_some_and(|s| matches!(s, "failed" | "interrupted"))
        {
            sqlx::query(
                "UPDATE routines SET enabled=FALSE, last_error='The previous assignment needs attention. Review it before resuming.', updated_at=NOW() WHERE id=$1",
            )
            .bind(&routine.id)
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        } else {
            match admit_routine_work(
                &mut tx,
                &routine,
                scheduled_for,
                "scheduled",
                None,
                true,
                now,
                None,
            )
            .await
            {
                Ok(Some(_)) => count += 1,
                Ok(None) => {}
                Err(ApiError::Internal(message)) => return Err(ApiError::Internal(message)),
                Err(_err) => {
                    sqlx::query(
                        "UPDATE routines SET enabled=FALSE, last_error='Could not schedule work. Check the bot, computer, destination, and pending work before resuming.', updated_at=NOW() WHERE id=$1",
                    )
                    .bind(&routine.id)
                    .execute(&mut *tx)
                    .await
                    .map_err(db_error)?;
                }
            }
        }
        tx.commit().await.map_err(db_error)?;
    }
    Ok(count)
}

pub async fn test_run(pool: &PgPool, owner: &str, id: &str, key: &str) -> Result<String, ApiError> {
    if key.len() > 80 {
        return Err(ApiError::Validation("Request key is too long".into()));
    }
    let mut tx = pool.begin().await.map_err(db_error)?;
    let routine: Routine =
        sqlx::query_as("SELECT * FROM routines WHERE id=$1 AND owner_id=$2 FOR UPDATE")
            .bind(id)
            .bind(owner)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_error)?
            .ok_or(ApiError::NotFound)?;

    if let Some(run_id) = sqlx::query_scalar::<_, Option<String>>(
        "SELECT run_id FROM routine_runs WHERE routine_id = $1 AND idempotency_key = $2",
    )
    .bind(id)
    .bind(key)
    .fetch_optional(&mut *tx)
    .await
    .map_err(db_error)?
    .flatten()
    {
        tx.commit().await.map_err(db_error)?;
        return Ok(run_id);
    }

    let busy = if let Some(last_run_id) = &routine.last_run_id {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM agent_runs WHERE id=$1 AND status IN ('queued','running'))",
        )
        .bind(last_run_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(db_error)?
    } else {
        false
    };
    if busy {
        return Err(ApiError::Conflict(
            "This routine already has work in progress".into(),
        ));
    }

    let now = Utc::now();
    let run_id = admit_routine_work(&mut tx, &routine, now, "test", Some(key), false, now, None)
        .await?
        .ok_or_else(|| ApiError::Conflict("Duplicate test run".into()))?;

    tx.commit().await.map_err(db_error)?;
    Ok(run_id)
}

pub async fn run_now(pool: &PgPool, owner: &str, id: &str, key: &str) -> Result<String, ApiError> {
    test_run(pool, owner, id, key).await
}

pub async fn list_runs(pool: &PgPool, owner: &str, id: &str) -> Result<Vec<RoutineRun>, ApiError> {
    routine_runs::list_for_routine(pool, owner, id, 20).await
}

pub async fn admit_webhook_event(
    pool: &PgPool,
    token: &str,
    payload: serde_json::Value,
    event_id: Option<&str>,
    event_source: Option<&str>,
) -> Result<String, ApiError> {
    if let Some(event_id) = event_id {
        if event_id.len() > 128 {
            return Err(ApiError::Validation("Event id is too long".into()));
        }
    }
    let mut tx = pool.begin().await.map_err(db_error)?;
    let Some((trigger, trigger_mode, enabled)) =
        routine_webhooks::lookup_by_token(&mut tx, token).await?
    else {
        return Err(ApiError::NotFound);
    };
    if trigger_mode != "webhook" || !enabled {
        return Err(ApiError::NotFound);
    }

    let routine: Routine = sqlx::query_as("SELECT * FROM routines WHERE id = $1 AND owner_id = $2")
        .bind(&trigger.routine_id)
        .bind(&trigger.owner_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(db_error)?;

    let previous: Option<String> = if let Some(id) = &routine.last_run_id {
        sqlx::query_scalar("SELECT status FROM agent_runs WHERE id=$1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_error)?
    } else {
        None
    };
    if routine.failure_policy == "pause_after_failure"
        && routine.acknowledged_run_id != routine.last_run_id
        && previous
            .as_deref()
            .is_some_and(|s| matches!(s, "failed" | "interrupted"))
    {
        sqlx::query(
            "UPDATE routines SET enabled=FALSE, last_error='The previous assignment needs attention. Review it before resuming.', updated_at=NOW() WHERE id=$1",
        )
        .bind(&routine.id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
        tx.commit().await.map_err(db_error)?;
        return Err(ApiError::NotFound);
    }

    let now = Utc::now();
    let (conversation_id, destination_label, _is_group) = destination_label_in_tx(
        &mut tx,
        &routine.owner_id,
        &routine.bot_id,
        routine.destination_conversation_id.as_deref(),
    )
    .await?;
    let _ = conversation_id;
    let runtime_message = routine_webhooks::compose_webhook_runtime_message(
        &routine.name,
        &routine.instructions,
        now,
        event_source,
        &payload,
        &destination_label,
    );

    let run_id = admit_routine_work(
        &mut tx,
        &routine,
        now,
        "webhook",
        event_id,
        false,
        now,
        Some(runtime_message),
    )
    .await?
    .ok_or_else(|| ApiError::Conflict("Duplicate webhook event".into()))?;

    routine_webhooks::mark_triggered_in_tx(&mut tx, &trigger.id, now).await?;
    tx.commit().await.map_err(db_error)?;
    Ok(run_id)
}
