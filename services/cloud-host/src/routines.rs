//! Recurring assignments reuse the normal work queue, ownership, and approvals.
use crate::error::ApiError;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

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
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoutineInput {
    pub bot_id: String,
    pub name: String,
    pub instructions: String,
    pub interval_minutes: i32,
    pub next_run_at: DateTime<Utc>,
    pub enabled: bool,
}

impl RoutineInput {
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
        if !(15..=43_200).contains(&self.interval_minutes) {
            return Err(ApiError::Validation(
                "Repeat intervals must be between 15 minutes and 30 days".into(),
            ));
        }
        if self.next_run_at > Utc::now() + Duration::days(366) {
            return Err(ApiError::Validation(
                "Choose a first run within the next year".into(),
            ));
        }
        Ok(())
    }
}

fn db_error(error: sqlx::Error) -> ApiError {
    ApiError::Internal(error.to_string())
}

pub async fn list(pool: &PgPool, owner: &str) -> Result<Vec<Routine>, ApiError> {
    sqlx::query_as("SELECT * FROM routines WHERE owner_id = $1 ORDER BY created_at DESC LIMIT 100")
        .bind(owner)
        .fetch_all(pool)
        .await
        .map_err(db_error)
}

pub async fn save(
    pool: &PgPool,
    owner: &str,
    id: Option<&str>,
    input: &RoutineInput,
) -> Result<Routine, ApiError> {
    input.validate()?;
    let mut tx = pool.begin().await.map_err(db_error)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(format!("routines-owner:{owner}"))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    if let Some(id) = id {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM routines WHERE id = $1 AND owner_id = $2)",
        )
        .bind(id)
        .bind(owner)
        .fetch_one(&mut *tx)
        .await
        .map_err(db_error)?;
        if !exists {
            return Err(ApiError::NotFound);
        }
    } else {
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
    let allowed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM bots b JOIN sandboxes s ON s.id = b.computer_id AND s.owner_id = b.owner_id WHERE b.id=$1 AND b.owner_id=$2 AND s.state <> 'archived')")
        .bind(&input.bot_id).bind(owner).fetch_one(&mut *tx).await.map_err(db_error)?;
    if !allowed {
        return Err(ApiError::Validation(
            "Choose one of your bots with an available computer".into(),
        ));
    }
    let new_id = id
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let row = sqlx::query_as("INSERT INTO routines (id,owner_id,bot_id,name,instructions,interval_minutes,next_run_at,enabled) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT (id) DO UPDATE SET bot_id=EXCLUDED.bot_id, name=EXCLUDED.name, instructions=EXCLUDED.instructions, interval_minutes=EXCLUDED.interval_minutes, next_run_at=EXCLUDED.next_run_at, enabled=EXCLUDED.enabled, acknowledged_run_id=CASE WHEN EXCLUDED.enabled THEN routines.last_run_id ELSE routines.acknowledged_run_id END, last_error=NULL, updated_at=NOW() WHERE routines.owner_id=EXCLUDED.owner_id RETURNING *")
        .bind(new_id).bind(owner).bind(&input.bot_id).bind(input.name.trim()).bind(input.instructions.trim()).bind(input.interval_minutes).bind(input.next_run_at).bind(input.enabled)
        .fetch_one(&mut *tx).await.map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    Ok(row)
}

pub async fn set_enabled(
    pool: &PgPool,
    owner: &str,
    id: &str,
    enabled: bool,
) -> Result<Routine, ApiError> {
    sqlx::query_as("UPDATE routines SET enabled=$3, acknowledged_run_id=CASE WHEN $3 THEN last_run_id ELSE acknowledged_run_id END, next_run_at=CASE WHEN $3 AND next_run_at < NOW() THEN NOW() + interval_minutes * INTERVAL '1 minute' ELSE next_run_at END, last_error=CASE WHEN $3 THEN NULL ELSE last_error END, updated_at=NOW() WHERE id=$1 AND owner_id=$2 RETURNING *")
        .bind(id).bind(owner).bind(enabled).fetch_optional(pool).await.map_err(db_error)?.ok_or(ApiError::NotFound)
}

/// Preserve cadence while coalescing missed occurrences into a single assignment.
pub fn next_occurrence(due: DateTime<Utc>, minutes: i32, now: DateTime<Utc>) -> DateTime<Utc> {
    let interval = i64::from(minutes) * 60;
    let missed = (now - due).num_seconds().max(0) / interval + 1;
    due + Duration::seconds(missed * interval)
}

pub async fn tick(pool: &PgPool, now: DateTime<Utc>) -> Result<usize, ApiError> {
    let mut count = 0;
    for _ in 0..20 {
        let mut tx = pool.begin().await.map_err(db_error)?;
        let routine: Option<Routine> = sqlx::query_as("SELECT * FROM routines WHERE enabled AND next_run_at <= $1 ORDER BY next_run_at LIMIT 1 FOR UPDATE SKIP LOCKED")
            .bind(now).fetch_optional(&mut *tx).await.map_err(db_error)?;
        let Some(routine) = routine else {
            break;
        };
        let previous: Option<String> = if let Some(id) = &routine.last_run_id {
            sqlx::query_scalar("SELECT status FROM agent_runs WHERE id=$1")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(db_error)?
        } else {
            None
        };
        let next = next_occurrence(routine.next_run_at, routine.interval_minutes, now);
        if previous
            .as_deref()
            .is_some_and(|s| matches!(s, "queued" | "running"))
        {
            sqlx::query("UPDATE routines SET next_run_at=$2, updated_at=NOW() WHERE id=$1")
                .bind(&routine.id)
                .bind(next)
                .execute(&mut *tx)
                .await
                .map_err(db_error)?;
        } else if routine.acknowledged_run_id != routine.last_run_id
            && previous
                .as_deref()
                .is_some_and(|s| matches!(s, "failed" | "interrupted"))
        {
            sqlx::query("UPDATE routines SET enabled=FALSE, last_error='The previous assignment needs attention. Review it before resuming.', updated_at=NOW() WHERE id=$1")
                .bind(&routine.id).execute(&mut *tx).await.map_err(db_error)?;
        } else {
            sqlx::query("SAVEPOINT routine_admission")
                .execute(&mut *tx)
                .await
                .map_err(db_error)?;
            let key = format!(
                "routine:{}:{}",
                routine.id,
                routine.next_run_at.timestamp_millis()
            );
            match crate::work::enqueue_in_transaction(
                &mut tx,
                &routine.owner_id,
                &key,
                &routine.bot_id,
                None,
                &routine.instructions,
            )
            .await
            {
                Ok(work) => {
                    sqlx::query("UPDATE work_queue SET routine_id=$2 WHERE run_id=$1")
                        .bind(&work.run_id)
                        .bind(&routine.id)
                        .execute(&mut *tx)
                        .await
                        .map_err(db_error)?;
                    sqlx::query("UPDATE routines SET last_run_id=$2,next_run_at=$3,last_error=NULL,updated_at=NOW() WHERE id=$1")
                        .bind(&routine.id).bind(work.run_id).bind(next).execute(&mut *tx).await.map_err(db_error)?;
                    count += 1;
                }
                Err(ApiError::Internal(message)) => return Err(ApiError::Internal(message)),
                Err(_) => {
                    sqlx::query("ROLLBACK TO SAVEPOINT routine_admission")
                        .execute(&mut *tx)
                        .await
                        .map_err(db_error)?;
                    sqlx::query("UPDATE routines SET enabled=FALSE,last_error='Could not schedule work. Check the bot, computer, and pending work before resuming.',updated_at=NOW() WHERE id=$1")
                        .bind(&routine.id).execute(&mut *tx).await.map_err(db_error)?;
                }
            }
        }
        tx.commit().await.map_err(db_error)?;
    }
    Ok(count)
}

pub async fn run_now(pool: &PgPool, owner: &str, id: &str, key: &str) -> Result<String, ApiError> {
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
    let request = format!("routine-manual:{id}:{key}");
    if let Some(run_id) = sqlx::query_scalar::<_, String>(
        "SELECT id FROM agent_runs WHERE request_id=$1 AND owner_id=$2",
    )
    .bind(&request)
    .bind(owner)
    .fetch_optional(&mut *tx)
    .await
    .map_err(db_error)?
    {
        return Ok(run_id);
    }
    let busy: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM agent_runs WHERE id=$1 AND status IN ('queued','running'))",
    )
    .bind(&routine.last_run_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(db_error)?;
    if busy {
        return Err(ApiError::Conflict(
            "This routine already has work in progress".into(),
        ));
    }
    let work = crate::work::enqueue_in_transaction(
        &mut tx,
        owner,
        &request,
        &routine.bot_id,
        None,
        &routine.instructions,
    )
    .await?;
    sqlx::query("UPDATE work_queue SET routine_id=$2 WHERE run_id=$1")
        .bind(&work.run_id)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    sqlx::query("UPDATE routines SET last_run_id=$2,last_error=NULL,updated_at=NOW() WHERE id=$1")
        .bind(id)
        .bind(&work.run_id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    Ok(work.run_id)
}
