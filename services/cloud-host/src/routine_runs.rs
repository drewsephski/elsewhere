//! Durable routine occurrence history synchronized with canonical agent runs.

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::error::ApiError;
use crate::run_lifecycle::is_terminal_run_status;

fn db_error(error: sqlx::Error) -> ApiError {
    ApiError::Internal(error.to_string())
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct RoutineRun {
    pub id: String,
    pub routine_id: String,
    pub run_id: Option<String>,
    pub scheduled_for: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub trigger_kind: String,
    pub created_at: DateTime<Utc>,
}

pub async fn list_for_routine(
    pool: &PgPool,
    owner: &str,
    routine_id: &str,
    limit: i64,
) -> Result<Vec<RoutineRun>, ApiError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM routines WHERE id = $1 AND owner_id = $2)",
    )
    .bind(routine_id)
    .bind(owner)
    .fetch_one(pool)
    .await
    .map_err(db_error)?;
    if !exists {
        return Err(ApiError::NotFound);
    }
    sqlx::query_as(
        r#"
        SELECT id, routine_id, run_id, scheduled_for, started_at, finished_at,
               status, error_code, error_message, trigger_kind, created_at
        FROM routine_runs
        WHERE routine_id = $1
        ORDER BY created_at DESC
        LIMIT $2
        "#,
    )
    .bind(routine_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(db_error)
}

pub async fn admit_occurrence_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    owner: &str,
    routine_id: &str,
    scheduled_for: DateTime<Utc>,
    trigger_kind: &str,
    idempotency_key: Option<&str>,
) -> Result<String, ApiError> {
    if let Some(key) = idempotency_key {
        if let Some(existing) = sqlx::query_scalar::<_, String>(
            "SELECT id FROM routine_runs WHERE routine_id = $1 AND idempotency_key = $2",
        )
        .bind(routine_id)
        .bind(key)
        .fetch_optional(&mut **tx)
        .await
        .map_err(db_error)?
        {
            return Ok(existing);
        }
    }
    if trigger_kind == "scheduled" {
        if let Some(existing) = sqlx::query_scalar::<_, String>(
            "SELECT id FROM routine_runs WHERE routine_id = $1 AND scheduled_for = $2 AND trigger_kind = 'scheduled'",
        )
        .bind(routine_id)
        .bind(scheduled_for)
        .fetch_optional(&mut **tx)
        .await
        .map_err(db_error)?
        {
            return Ok(existing);
        }
    }
    let id = Uuid::new_v4().to_string();
    if let Err(err) = sqlx::query(
        r#"
        INSERT INTO routine_runs (
            id, routine_id, owner_id, scheduled_for, status, trigger_kind, idempotency_key
        ) VALUES ($1, $2, $3, $4, 'queued', $5, $6)
        "#,
    )
    .bind(&id)
    .bind(routine_id)
    .bind(owner)
    .bind(scheduled_for)
    .bind(trigger_kind)
    .bind(idempotency_key)
    .execute(&mut **tx)
    .await
    {
        if is_unique_violation(&err) {
            if trigger_kind == "scheduled" {
                let existing: String = sqlx::query_scalar(
                    "SELECT id FROM routine_runs WHERE routine_id = $1 AND scheduled_for = $2 AND trigger_kind = 'scheduled'",
                )
                .bind(routine_id)
                .bind(scheduled_for)
                .fetch_one(&mut **tx)
                .await
                .map_err(db_error)?;
                return Ok(existing);
            }
            if let Some(key) = idempotency_key {
                let existing: String = sqlx::query_scalar(
                    "SELECT id FROM routine_runs WHERE routine_id = $1 AND idempotency_key = $2",
                )
                .bind(routine_id)
                .bind(key)
                .fetch_one(&mut **tx)
                .await
                .map_err(db_error)?;
                return Ok(existing);
            }
            return Err(ApiError::Conflict("Duplicate routine request".into()));
        }
        return Err(db_error(err));
    }
    Ok(id)
}

pub async fn record_skipped_scheduled_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    owner: &str,
    routine_id: &str,
    scheduled_for: DateTime<Utc>,
    error_code: &str,
    error_message: &str,
) -> Result<(), ApiError> {
    if let Some(existing) = sqlx::query_scalar::<_, String>(
        "SELECT id FROM routine_runs WHERE routine_id = $1 AND scheduled_for = $2 AND trigger_kind = 'scheduled'",
    )
    .bind(routine_id)
    .bind(scheduled_for)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    {
        sqlx::query(
            r#"
            UPDATE routine_runs
            SET status = 'skipped',
                finished_at = NOW(),
                error_code = $2,
                error_message = $3
            WHERE id = $1 AND status NOT IN ('completed', 'failed', 'cancelled', 'skipped')
            "#,
        )
        .bind(&existing)
        .bind(error_code)
        .bind(error_message)
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
        return Ok(());
    }

    let id = Uuid::new_v4().to_string();
    if let Err(err) = sqlx::query(
        r#"
        INSERT INTO routine_runs (
            id, routine_id, owner_id, scheduled_for, status, trigger_kind,
            finished_at, error_code, error_message
        ) VALUES ($1, $2, $3, $4, 'skipped', 'scheduled', NOW(), $5, $6)
        "#,
    )
    .bind(&id)
    .bind(routine_id)
    .bind(owner)
    .bind(scheduled_for)
    .bind(error_code)
    .bind(error_message)
    .execute(&mut **tx)
    .await
    {
        if is_unique_violation(&err) {
            return Ok(());
        }
        return Err(db_error(err));
    }
    Ok(())
}

pub async fn link_run_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    occurrence_id: &str,
    run_id: &str,
) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE routine_runs SET run_id = $2 WHERE id = $1 AND run_id IS NULL",
    )
    .bind(occurrence_id)
    .bind(run_id)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    Ok(())
}

pub async fn mark_running_for_run(pool: &PgPool, run_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE routine_runs
        SET status = 'running',
            started_at = COALESCE(started_at, NOW())
        WHERE run_id = $1 AND status = 'queued'
        "#,
    )
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn sync_terminal_for_run_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    run_id: &str,
    run_status: &str,
    error_code: Option<&str>,
) -> Result<(), sqlx::Error> {
    if !is_terminal_run_status(run_status) {
        return Ok(());
    }
    let row = sqlx::query(
        r#"
        SELECT rr.id, rr.routine_id, rr.status, rr.trigger_kind
        FROM routine_runs rr
        WHERE rr.run_id = $1
        FOR UPDATE
        "#,
    )
    .bind(run_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some(row) = row else {
        return Ok(());
    };

    let occurrence_id: String = row.get("id");
    let routine_id: String = row.get("routine_id");
    let current: String = row.get("status");
    if matches!(current.as_str(), "completed" | "failed" | "cancelled" | "skipped") {
        return Ok(());
    }

    let (occurrence_status, routine_success) = match run_status {
        "completed" => ("completed", true),
        "cancelled" => ("cancelled", false),
        _ => ("failed", false),
    };

    sqlx::query(
        r#"
        UPDATE routine_runs
        SET status = $2,
            finished_at = NOW(),
            error_code = $3,
            error_message = CASE WHEN $2 = 'failed' THEN COALESCE($4, error_message) ELSE NULL END
        WHERE id = $1
        "#,
    )
    .bind(&occurrence_id)
    .bind(occurrence_status)
    .bind(error_code)
    .bind(error_code)
    .execute(&mut **tx)
    .await?;

    if routine_success {
        sqlx::query(
            r#"
            UPDATE routines
            SET last_success_at = NOW(),
                consecutive_failures = 0,
                last_error = NULL,
                acknowledged_run_id = last_run_id,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(&routine_id)
        .execute(&mut **tx)
        .await?;
    } else if occurrence_status == "failed" {
        sqlx::query(
            r#"
            UPDATE routines
            SET last_failure_at = NOW(),
                consecutive_failures = consecutive_failures + 1,
                last_error = COALESCE($2, last_error),
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(&routine_id)
        .bind(error_code)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(
        err,
        sqlx::Error::Database(db) if db.code().as_deref() == Some("23505")
    )
}

pub async fn reconcile_stale_routine_runs(pool: &PgPool) -> Result<(), sqlx::Error> {
    let stale: Vec<(String, String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT rr.run_id, r.status, r.error_code
        FROM routine_runs rr
        JOIN agent_runs r ON r.id = rr.run_id
        WHERE rr.status IN ('queued', 'running')
          AND r.status IN ('completed', 'failed', 'cancelled', 'interrupted')
        "#,
    )
    .fetch_all(pool)
    .await?;

    for (run_id, status, error_code) in stale {
        let mut tx = pool.begin().await?;
        sync_terminal_for_run_in_tx(&mut tx, &run_id, &status, error_code.as_deref()).await?;
        tx.commit().await?;
    }
    Ok(())
}
