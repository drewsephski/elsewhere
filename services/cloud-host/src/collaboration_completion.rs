//! Public orchestration seam for post-run result collection and delegation completion.

use std::sync::Arc;
use std::time::Duration;

use agent_core::AgentComputer;
use sqlx::PgPool;
use tokio::time::timeout;

use crate::app_state::AppState;

/// After a successful agent execution, finalize durable results and advance delegation return.
pub async fn on_run_terminal_success(
    state: &AppState,
    pool: &PgPool,
    run_id: &str,
    computer: Arc<dyn AgentComputer>,
) {
    let run_status: String = sqlx::query_scalar("SELECT status FROM agent_runs WHERE id = $1")
        .bind(run_id)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|_| "completed".into());

    if run_status != "completed" {
        return;
    }

    on_target_results_finalized(state, pool, run_id, computer).await;
}

/// Result collection policy for a completed target run, then delegation artifact/return stages.
pub async fn on_target_results_finalized(
    state: &AppState,
    pool: &PgPool,
    run_id: &str,
    computer: Arc<dyn AgentComputer>,
) {
    let _ = crate::result_finalization::begin_collecting(pool, run_id).await;
    let collect_outcome = timeout(
        Duration::from_secs(30),
        crate::results::collect(pool, run_id, computer.as_ref()),
    )
    .await;

    let (results_status, note) = match collect_outcome {
        Ok(Ok(())) => ("complete", None),
        Ok(Err(error)) => {
            tracing::warn!(run_id = %run_id, error = %error, "result collection incomplete");
            (
                crate::result_finalization::collection_status_from_collect_error(&error),
                Some(error),
            )
        }
        Err(_) => (
            "failed",
            Some(
                "Saving file results timed out. Check the summary and files on the computer."
                    .into(),
            ),
        ),
    };

    if let Err(error) = crate::result_finalization::finalize_collection(
        pool,
        run_id,
        results_status,
        note.as_deref(),
    )
    .await
    {
        tracing::error!(
            run_id = %run_id,
            error = %error,
            "could not finalize result collection"
        );
    }

    advance_delegation_after_target_results(state, pool, run_id).await;
}

/// Artifact transfer and source continuation admission after target results are terminal.
pub async fn advance_delegation_after_target_results(
    state: &AppState,
    pool: &PgPool,
    target_run_id: &str,
) {
    let delegation_id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM bot_delegations WHERE target_run_id = $1 AND return_policy = 'resume_source'",
    )
    .bind(target_run_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    if let Some(delegation_id) = delegation_id {
        if let Err(err) =
            crate::artifact_handoff::execute_pending_transfers_for_delegation(state, &delegation_id)
                .await
        {
            tracing::warn!(
                delegation_id = %delegation_id,
                error = %err,
                "artifact handoff execution failed"
            );
        }
        if let Err(err) =
            crate::run_lifecycle::try_admit_delegation_return(pool, &delegation_id).await
        {
            tracing::warn!(
                delegation_id = %delegation_id,
                error = %err,
                "delegation return admission failed"
            );
        }
    }
}

/// Worker/reconciliation entry: repair artifact transfer and return admission without re-running collection.
pub async fn reconcile_delegation_completion(state: &AppState, delegation_id: &str) -> Result<(), String> {
    if let Err(err) =
        crate::artifact_handoff::execute_pending_transfers_for_delegation(state, delegation_id)
            .await
    {
        return Err(err.to_string());
    }
    crate::run_lifecycle::try_admit_delegation_return(&state.pool, delegation_id)
        .await
        .map_err(|e| e.to_string())
}
