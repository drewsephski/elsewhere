//! Canonical terminal run synchronization: delegations, group recipients, resume lifecycle.

use serde_json::json;
use sqlx::{PgPool, Postgres, Row, Transaction};

pub fn is_terminal_run_status(status: &str) -> bool {
    matches!(
        status,
        "completed" | "failed" | "cancelled" | "interrupted"
    )
}

pub fn map_run_status_to_recipient_status(run_status: &str) -> &'static str {
    match run_status {
        "completed" => "completed",
        "cancelled" => "cancelled",
        _ => "failed",
    }
}

pub fn bots_share_computer(source_computer_id: &str, target_computer_id: &str) -> bool {
    source_computer_id == target_computer_id
}

#[derive(Debug, Default)]
struct RepairCounters {
    delegation_terminal_repairs: u64,
    delegation_resume_repairs: u64,
    group_recipient_repairs: u64,
}

fn log_repair(
    delegation_id: Option<&str>,
    target_run_id: Option<&str>,
    source_resume_run_id: Option<&str>,
    old_delegation_status: Option<&str>,
    target_run_status: &str,
    resume_status: Option<&str>,
    repair_action: &str,
) {
    tracing::info!(
        target = "elsewhere_collaboration_repair",
        delegation_id = delegation_id.unwrap_or(""),
        target_run_id = target_run_id.unwrap_or(""),
        source_resume_run_id = source_resume_run_id.unwrap_or(""),
        old_delegation_status = old_delegation_status.unwrap_or(""),
        target_run_status,
        resume_status = resume_status.unwrap_or(""),
        repair_action,
        "collaboration lifecycle repair"
    );
}

async fn insert_source_return_event_if_absent(
    tx: &mut Transaction<'_, Postgres>,
    source_request_id: &str,
    event_type: &str,
    delegation_id: &str,
    payload_extra: serde_json::Value,
) -> Result<(), sqlx::Error> {
    let exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
          SELECT 1 FROM run_events
          WHERE request_id = $1
            AND event_type = $2
            AND payload_json->>'delegationId' = $3
        )
        "#,
    )
    .bind(source_request_id)
    .bind(event_type)
    .bind(delegation_id)
    .fetch_one(&mut **tx)
    .await?;
    if exists {
        return Ok(());
    }
    let mut payload = json!({ "delegationId": delegation_id });
    if let Some(obj) = payload_extra.as_object() {
        for (k, v) in obj {
            payload[k] = v.clone();
        }
    }
    sqlx::query(
        "INSERT INTO run_events (request_id, event_type, payload_json) VALUES ($1, $2, $3)",
    )
    .bind(source_request_id)
    .bind(event_type)
    .bind(&payload)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn sync_group_recipients_for_run_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    run_id: &str,
    run_status: &str,
    counters: &mut RepairCounters,
) -> Result<(), sqlx::Error> {
    if !is_terminal_run_status(run_status) {
        return Ok(());
    }
    let recipient_status = map_run_status_to_recipient_status(run_status);
    let updated = sqlx::query(
        r#"
        UPDATE group_message_recipients
        SET status = $2, updated_at = NOW()
        WHERE run_id = $1 AND status IN ('queued', 'running')
        "#,
    )
    .bind(run_id)
    .bind(recipient_status)
    .execute(&mut **tx)
    .await?;
    if updated.rows_affected() > 0 {
        counters.group_recipient_repairs += updated.rows_affected();
        log_repair(
            None,
            Some(run_id),
            None,
            None,
            run_status,
            None,
            "group_recipient_terminal_sync",
        );
    }
    Ok(())
}

fn delegation_terminal_from_run(
    run_status: &str,
    error_code: Option<&str>,
) -> (String, String, Option<String>, Option<String>) {
    match run_status {
        "completed" => (
            "completed".into(),
            "bot_delegation_completed".into(),
            None,
            None,
        ),
        "cancelled" => (
            "cancelled".into(),
            "bot_delegation_failed".into(),
            Some("cancelled".into()),
            Some("cancelled".into()),
        ),
        "interrupted" => (
            "failed".into(),
            "bot_delegation_failed".into(),
            Some("interrupted".into()),
            Some("interrupted".into()),
        ),
        _ => (
            "failed".into(),
            "bot_delegation_failed".into(),
            error_code.map(str::to_string),
            error_code.map(str::to_string),
        ),
    }
}

async fn sync_delegation_target_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    target_run_id: &str,
    run_status: &str,
    error_code: Option<&str>,
    counters: &mut RepairCounters,
) -> Result<(), sqlx::Error> {
    if !is_terminal_run_status(run_status) {
        return Ok(());
    }

    let row = sqlx::query(
        r#"
        SELECT id, source_request_id, status, return_policy
        FROM bot_delegations
        WHERE target_run_id = $1
        FOR UPDATE
        "#,
    )
    .bind(target_run_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some(row) = row else {
        return Ok(());
    };

    let delegation_id: String = row.get("id");
    let source_request_id: String = row.get("source_request_id");
    let current: String = row.get("status");
    let return_policy: String = row.get("return_policy");

    let (delegation_status, event_type, err_code, err_message) =
        delegation_terminal_from_run(run_status, error_code);

    if !matches!(current.as_str(), "completed" | "failed" | "cancelled") {
        sqlx::query(
            r#"
            UPDATE bot_delegations
            SET status = $2,
                finished_at = NOW(),
                error_code = $3,
                error_message = $4
            WHERE id = $1
            "#,
        )
        .bind(&delegation_id)
        .bind(&delegation_status)
        .bind(err_code.as_deref())
        .bind(err_message.as_deref())
        .execute(&mut **tx)
        .await?;

        let payload = json!({
            "delegationId": delegation_id,
            "targetRunId": target_run_id,
            "status": delegation_status,
            "errorCode": err_code
        });
        sqlx::query(
            "INSERT INTO run_events (request_id, event_type, payload_json) VALUES ($1, $2, $3)",
        )
        .bind(&source_request_id)
        .bind(&event_type)
        .bind(&payload)
        .execute(&mut **tx)
        .await?;

        counters.delegation_terminal_repairs += 1;
        log_repair(
            Some(&delegation_id),
            Some(target_run_id),
            None,
            Some(&current),
            run_status,
            None,
            "delegation_target_terminal_sync",
        );
    }

    if return_policy == "resume_source"
        && try_admit_delegation_return_in_tx(tx, &delegation_id).await?
    {
        counters.delegation_resume_repairs += 1;
    }

    Ok(())
}

/// After target result collection + artifact handoff, enqueue source continuation when ready.
pub async fn try_admit_delegation_return(
    pool: &PgPool,
    delegation_id: &str,
) -> Result<(), sqlx::Error> {
    crate::artifact_handoff::plan_transfers_for_delegation(pool, delegation_id).await?;
    let mut tx = pool.begin().await?;
    if try_admit_delegation_return_in_tx(&mut tx, delegation_id).await? {
        tx.commit().await?;
    } else {
        tx.rollback().await.ok();
    }
    Ok(())
}

async fn try_admit_delegation_return_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    delegation_id: &str,
) -> Result<bool, sqlx::Error> {
    if !crate::artifact_handoff::delegation_return_ready_in_tx(tx, delegation_id).await? {
        return Ok(false);
    }
    match crate::work::enqueue_delegation_return_in_transaction(tx, delegation_id).await {
        Ok(Some(_)) => Ok(true),
        Ok(None) => Ok(false),
        Err(err) => {
            tracing::warn!(
                delegation_id = %delegation_id,
                error = %err,
                "could not enqueue delegation return"
            );
            Ok(false)
        }
    }
}

/// Target run results reached a terminal collection state; plan handoff and maybe admit return.
pub async fn on_target_results_finalized(pool: &PgPool, target_run_id: &str) -> Result<(), sqlx::Error> {
    let delegation_id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM bot_delegations WHERE target_run_id = $1 AND return_policy = 'resume_source'",
    )
    .bind(target_run_id)
    .fetch_optional(pool)
    .await?;

    let Some(delegation_id) = delegation_id else {
        return Ok(());
    };

    crate::artifact_handoff::plan_transfers_for_delegation(pool, &delegation_id).await?;
    Ok(())
}

async fn sync_delegation_return_run_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    resume_run_id: &str,
    run_status: &str,
    error_code: Option<&str>,
    _counters: &mut RepairCounters,
) -> Result<(), sqlx::Error> {
    if !is_terminal_run_status(run_status) {
        return Ok(());
    }

    let row = sqlx::query(
        r#"
        SELECT d.id, d.source_request_id, d.resume_status, d.source_resume_run_id
        FROM bot_delegations d
        WHERE d.source_resume_run_id = $1
        FOR UPDATE OF d
        "#,
    )
    .bind(resume_run_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some(row) = row else {
        return Ok(());
    };

    let delegation_id: String = row.get("id");
    let source_request_id: String = row.get("source_request_id");
    let current_resume: Option<String> = row.get("resume_status");

    let (next_status, event_type, resume_error) = match run_status {
        "completed" => ("completed", "bot_delegation_return_completed", None),
        "cancelled" => (
            "failed",
            "bot_delegation_return_failed",
            Some("cancelled"),
        ),
        "interrupted" => (
            "failed",
            "bot_delegation_return_failed",
            Some(error_code.unwrap_or("interrupted")),
        ),
        _ => (
            "failed",
            "bot_delegation_return_failed",
            Some(error_code.unwrap_or("failed")),
        ),
    };

    if current_resume.as_deref() == Some(next_status) {
        return Ok(());
    }

    sqlx::query(
        r#"
        UPDATE bot_delegations
        SET resume_status = $2,
            resume_error = $3
        WHERE id = $1
        "#,
    )
    .bind(&delegation_id)
    .bind(next_status)
    .bind(resume_error)
    .execute(&mut **tx)
    .await?;

    insert_source_return_event_if_absent(
        tx,
        &source_request_id,
        event_type,
        &delegation_id,
        json!({
            "resumeStatus": next_status,
            "sourceResumeRunId": resume_run_id,
            "errorCode": resume_error,
        }),
    )
    .await?;

    log_repair(
        Some(&delegation_id),
        None,
        Some(resume_run_id),
        None,
        run_status,
        Some(next_status),
        "delegation_return_terminal_sync",
    );

    Ok(())
}

/// Synchronize all dependent lifecycle state for a run that is already persisted as terminal.
pub async fn synchronize_run_terminal_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    run_id: &str,
    run_status: &str,
    error_code: Option<&str>,
) -> Result<(), sqlx::Error> {
    if !is_terminal_run_status(run_status) {
        return Ok(());
    }
    let mut counters = RepairCounters::default();
    sync_group_recipients_for_run_in_tx(tx, run_id, run_status, &mut counters).await?;
    sync_delegation_target_in_tx(tx, run_id, run_status, error_code, &mut counters).await?;
    sync_delegation_return_run_in_tx(tx, run_id, run_status, error_code, &mut counters).await?;
    Ok(())
}

pub async fn synchronize_run_terminal_for_request(
    pool: &PgPool,
    request_id: &str,
    run_status: &str,
    error_code: Option<&str>,
) -> Result<(), sqlx::Error> {
    if !is_terminal_run_status(run_status) {
        return Ok(());
    }
    let run_id: Option<String> =
        sqlx::query_scalar("SELECT id FROM agent_runs WHERE request_id = $1")
            .bind(request_id)
            .fetch_optional(pool)
            .await?;
    let Some(run_id) = run_id else {
        return Ok(());
    };
    let mut tx = pool.begin().await?;
    synchronize_run_terminal_in_tx(&mut tx, &run_id, run_status, error_code).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn reconcile_collaboration_lifecycle(pool: &PgPool) -> Result<(), sqlx::Error> {
    reconcile_stale_result_finalization(pool).await?;
    let stale_targets: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT tr.request_id, tr.id, tr.status, tr.error_code
        FROM bot_delegations d
        JOIN agent_runs tr ON tr.id = d.target_run_id
        WHERE tr.status IN ('completed', 'failed', 'cancelled', 'interrupted')
          AND d.status IN ('queued', 'running')
        "#,
    )
    .fetch_all(pool)
    .await?;

    for (request_id, run_id, status, error_code) in stale_targets {
        let mut tx = pool.begin().await?;
        synchronize_run_terminal_in_tx(&mut tx, &run_id, &status, error_code.as_deref()).await?;
        tx.commit().await?;
        let _ = request_id;
    }

    let pending_returns: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT d.id
        FROM bot_delegations d
        JOIN agent_runs tr ON tr.id = d.target_run_id
        WHERE d.return_policy = 'resume_source'
          AND tr.status IN ('completed', 'failed', 'cancelled', 'interrupted')
          AND d.status IN ('completed', 'failed', 'cancelled')
          AND d.source_resume_run_id IS NULL
          AND COALESCE(d.resume_status, '') NOT IN ('skipped', 'running', 'completed', 'failed')
        "#,
    )
    .fetch_all(pool)
    .await?;

    for delegation_id in pending_returns {
        crate::artifact_handoff::plan_transfers_for_delegation(pool, &delegation_id).await?;
        let mut tx = pool.begin().await?;
        if !crate::artifact_handoff::delegation_return_ready_in_tx(&mut tx, &delegation_id)
            .await?
        {
            tx.rollback().await.ok();
            continue;
        }
        if !try_admit_delegation_return_in_tx(&mut tx, &delegation_id).await? {
            tx.rollback().await.ok();
            continue;
        }
        tx.commit().await?;
    }

    let stale_recipients: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT r.id, r.status
        FROM group_message_recipients g
        JOIN agent_runs r ON r.id = g.run_id
        WHERE r.status IN ('completed', 'failed', 'cancelled', 'interrupted')
          AND g.status IN ('queued', 'running')
        "#,
    )
    .fetch_all(pool)
    .await?;

    for (run_id, status) in stale_recipients {
        let mut tx = pool.begin().await?;
        let mut counters = RepairCounters::default();
        sync_group_recipients_for_run_in_tx(&mut tx, &run_id, &status, &mut counters).await?;
        tx.commit().await?;
    }

    let stale_resume: Vec<(String, String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT sr.id, sr.status, sr.error_code
        FROM bot_delegations d
        JOIN agent_runs sr ON sr.id = d.source_resume_run_id
        WHERE d.resume_status IN ('queued', 'running')
          AND sr.status IN ('completed', 'failed', 'cancelled', 'interrupted')
        "#,
    )
    .fetch_all(pool)
    .await?;

    for (run_id, status, error_code) in stale_resume {
        let mut tx = pool.begin().await?;
        let mut counters = RepairCounters::default();
        sync_delegation_return_run_in_tx(
            &mut tx,
            &run_id,
            &status,
            error_code.as_deref(),
            &mut counters,
        )
        .await?;
        tx.commit().await?;
    }

    Ok(())
}

async fn reconcile_stale_result_finalization(pool: &PgPool) -> Result<(), sqlx::Error> {
    let stale: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT id FROM agent_runs
        WHERE status = 'completed'
          AND results_status IN ('pending', 'collecting')
          AND execution_released_at IS NOT NULL
        "#,
    )
    .fetch_all(pool)
    .await?;

    for run_id in stale {
        if let Err(err) = crate::result_finalization::finalize_collection(
            pool,
            &run_id,
            "failed",
            Some(
                "Result collection did not finish before the computer was released. Some files may remain only on the target computer.",
            ),
        )
        .await
        {
            tracing::warn!(
                run_id = %run_id,
                error = %err,
                repair_action = "stale_result_finalization",
                "could not finalize stale result collection"
            );
        }
    }
    Ok(())
}

pub async fn on_delegation_return_run_claimed(pool: &PgPool, run_id: &str) -> Result<(), sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT d.id, d.source_request_id, d.resume_status
        FROM bot_delegations d
        WHERE d.source_resume_run_id = $1
        "#,
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(());
    };

    let delegation_id: String = row.get("id");
    let source_request_id: String = row.get("source_request_id");
    let resume_status: Option<String> = row.get("resume_status");
    if resume_status.as_deref() == Some("running") {
        return Ok(());
    }

    sqlx::query(
        "UPDATE bot_delegations SET resume_status = 'running' WHERE id = $1 AND resume_status = 'queued'",
    )
    .bind(&delegation_id)
    .execute(pool)
    .await?;

    let mut tx = pool.begin().await?;
    insert_source_return_event_if_absent(
        &mut tx,
        &source_request_id,
        "bot_delegation_return_running",
        &delegation_id,
        json!({ "sourceResumeRunId": run_id, "resumeStatus": "running" }),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
