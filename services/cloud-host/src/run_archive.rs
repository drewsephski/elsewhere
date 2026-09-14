use sqlx::{PgPool, Row};

use codex_provider::{CodexAppServerClient, CodexProcessLaunch};

use crate::config::Config;
use crate::error::ApiError;
use crate::provider_profile;

const ACTIVE_STATUSES: &[&str] = &["queued", "running"];

pub async fn archive_run(
    pool: &PgPool,
    config: &Config,
    owner_id: &str,
    run_id: &str,
) -> Result<(), ApiError> {
    let row = sqlx::query(
        "SELECT request_id, status, archived_at FROM agent_runs WHERE id = $1 AND owner_id = $2",
    )
    .bind(run_id)
    .bind(owner_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or(ApiError::NotFound)?;

    let request_id: String = row.get("request_id");
    let status: String = row.get("status");
    let archived_at: Option<chrono::DateTime<chrono::Utc>> = row.get("archived_at");

    if archived_at.is_some() {
        return Ok(());
    }

    if ACTIVE_STATUSES.contains(&status.as_str()) {
        return Err(ApiError::Validation(
            "Stop or wait for work to finish before archiving".into(),
        ));
    }

    if let Some(thread_id) = codex_thread_id_for_request(pool, &request_id).await? {
        archive_codex_thread_best_effort(pool, config, owner_id, &thread_id).await;
    }

    let updated = sqlx::query(
        "UPDATE agent_runs SET archived_at = NOW(), updated_at = NOW() \
         WHERE id = $1 AND owner_id = $2 AND archived_at IS NULL",
    )
    .bind(run_id)
    .bind(owner_id)
    .execute(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    if updated.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }

    Ok(())
}

async fn codex_thread_id_for_request(
    pool: &PgPool,
    request_id: &str,
) -> Result<Option<String>, ApiError> {
    let thread_id: Option<String> = sqlx::query_scalar(
        "SELECT payload_json #>> '{provider,threadId}' FROM run_events \
         WHERE request_id = $1 AND event_type = 'run_started' \
         AND payload_json #>> '{provider,threadId}' IS NOT NULL \
         ORDER BY id ASC LIMIT 1",
    )
    .bind(request_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(thread_id.filter(|value| !value.trim().is_empty()))
}

async fn archive_codex_thread_best_effort(
    pool: &PgPool,
    config: &Config,
    owner_id: &str,
    thread_id: &str,
) {
    let Some(executable) = config.codex_executable.clone() else {
        return;
    };

    let profile = match provider_profile::profile_for_owner(pool, config, owner_id).await {
        Ok(profile) => profile,
        Err(err) => {
            tracing::debug!(error = %err, "skipping Codex thread archive; profile unavailable");
            return;
        }
    };

    let mut launch = CodexProcessLaunch::from_path(executable).subscription_child();
    if let Some(profile) = profile {
        launch = launch.with_profile(&profile);
    }

    let client = match CodexAppServerClient::launch(launch).await {
        Ok(client) => client,
        Err(err) => {
            tracing::warn!(error = %err, thread_id, "Codex client unavailable for thread archive");
            return;
        }
    };

    if let Err(err) = client.thread_archive(thread_id).await {
        tracing::warn!(error = %err, thread_id, "Codex thread/archive failed; run still archived locally");
    }

    if let Err(err) = client.shutdown().await {
        tracing::debug!(error = %err, "Codex client shutdown after thread archive");
    }
}
