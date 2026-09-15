//! Result collection lifecycle on `agent_runs` (ordering before delegation artifact handoff).

use sqlx::{PgPool, Postgres, Transaction};

pub fn is_results_status_terminal(status: &str) -> bool {
    matches!(status, "complete" | "partial" | "failed" | "skipped")
}

pub async fn mark_terminal_run_results_policy(
    tx: &mut Transaction<'_, Postgres>,
    run_id: &str,
    run_status: &str,
) -> Result<(), sqlx::Error> {
    if run_status == "completed" {
        sqlx::query(
            r#"
            UPDATE agent_runs
            SET results_status = COALESCE(results_status, 'pending')
            WHERE id = $1
            "#,
        )
        .bind(run_id)
        .execute(&mut **tx)
        .await?;
    } else if matches!(run_status, "failed" | "cancelled" | "interrupted") {
        sqlx::query(
            r#"
            UPDATE agent_runs
            SET results_status = 'skipped',
                results_finalized_at = COALESCE(results_finalized_at, NOW())
            WHERE id = $1 AND results_status IS NULL
            "#,
        )
        .bind(run_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub async fn begin_collecting(pool: &PgPool, run_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE agent_runs
        SET results_status = 'collecting'
        WHERE id = $1
          AND status = 'completed'
          AND results_status IN ('pending', 'collecting')
        "#,
    )
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn finalize_collection(
    pool: &PgPool,
    run_id: &str,
    results_status: &str,
    results_note: Option<&str>,
) -> Result<(), sqlx::Error> {
    if !is_results_status_terminal(results_status) {
        return Ok(());
    }
    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"
        UPDATE agent_runs
        SET results_status = $2,
            results_finalized_at = COALESCE(results_finalized_at, NOW()),
            results_note = COALESCE($3, results_note)
        WHERE id = $1
          AND status = 'completed'
        "#,
    )
    .bind(run_id)
    .bind(results_status)
    .bind(results_note)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    if let Err(err) = crate::run_lifecycle::on_target_results_finalized(pool, run_id).await {
        tracing::warn!(
            run_id = %run_id,
            error = %err,
            "post-result-finalization collaboration hook failed"
        );
    }
    Ok(())
}

pub fn collection_status_from_collect_error(error: &str) -> &'static str {
    if error.contains("Some files remain") {
        "partial"
    } else {
        "failed"
    }
}
