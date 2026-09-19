//! Server-mediated artifact relay from target `work_results` to source Bot computers.

use std::time::Duration;

use agent_core::AgentComputer;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::error::ApiError;
use crate::results::{self, safe_name, COUNT_LIMIT, FILE_LIMIT, TOTAL_LIMIT};

pub const DELEGATION_ARTIFACT_ROOT: &str = "/workspace/shared/delegations";
pub const ARTIFACT_TRANSFER_LEASE: Duration = Duration::from_secs(120);

#[derive(Debug, Clone)]
pub struct ArtifactContextLine {
    pub name: String,
    pub path: Option<String>,
    pub transfer_status: String,
    pub error: Option<String>,
}

pub fn delegation_handoff_directory(delegation_id: &str) -> Option<String> {
    if !safe_delegation_path_component(delegation_id) {
        return None;
    }
    Some(format!("{DELEGATION_ARTIFACT_ROOT}/{delegation_id}"))
}

pub fn destination_path_for_file(delegation_id: &str, filename: &str) -> Option<String> {
    if !safe_name(filename) {
        return None;
    }
    delegation_handoff_directory(delegation_id).map(|dir| format!("{dir}/{filename}"))
}

fn safe_delegation_path_component(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn bump_workspace_revision(computer: &dyn AgentComputer) {
    computer.record_workspace_mutation(
        "workspace_write",
        &json!({ "ok": true, "source": "artifact_handoff" }),
    );
}

pub async fn plan_transfers_for_delegation(
    pool: &PgPool,
    delegation_id: &str,
) -> Result<(), sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT d.id, d.owner_id, d.target_run_id,
               sb.computer_id AS source_computer_id,
               tb.computer_id AS target_computer_id,
               tr.status AS target_run_status
        FROM bot_delegations d
        JOIN bots sb ON sb.id = d.source_bot_id
        JOIN bots tb ON tb.id = d.target_bot_id
        JOIN agent_runs tr ON tr.id = d.target_run_id
        WHERE d.id = $1
        "#,
    )
    .bind(delegation_id)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(());
    };

    let target_run_status: String = row.get("target_run_status");
    if target_run_status != "completed" {
        return Ok(());
    }

    let target_run_id: String = row.get("target_run_id");
    let source_computer_id: String = row.get("source_computer_id");
    let target_computer_id: String = row.get("target_computer_id");
    let shared =
        crate::run_lifecycle::bots_share_computer(&source_computer_id, &target_computer_id);

    let files = sqlx::query(
        r#"
        SELECT id, name, octet_length(content)::bigint AS size
        FROM work_results
        WHERE run_id = $1 AND kind = 'file'
        ORDER BY created_at ASC, name ASC
        LIMIT $2
        "#,
    )
    .bind(&target_run_id)
    .bind(COUNT_LIMIT as i64)
    .fetch_all(pool)
    .await?;

    let mut total: usize = 0;
    for file in files {
        let result_id: Uuid = file.get("id");
        let name: String = file.get("name");
        let size: i64 = file.get("size");
        if !safe_name(&name) || size > FILE_LIMIT as i64 {
            continue;
        }
        if total + size as usize > TOTAL_LIMIT {
            break;
        }
        total += size as usize;

        let (status, destination_path, skip_reason) = if shared {
            let path = format!("{}/{}", results::output_directory(&target_run_id), name);
            ("skipped", path, Some("shared_computer"))
        } else if let Some(path) = destination_path_for_file(delegation_id, &name) {
            ("pending", path, None)
        } else {
            continue;
        };

        sqlx::query(
            r#"
            INSERT INTO delegation_artifact_transfers (
                id, delegation_id, source_result_id, source_run_id,
                source_computer_id, destination_computer_id, destination_path,
                name, size, status, skip_reason
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
            ON CONFLICT (delegation_id, source_result_id) DO NOTHING
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(delegation_id)
        .bind(result_id)
        .bind(&target_run_id)
        .bind(&source_computer_id)
        .bind(if shared {
            &target_computer_id
        } else {
            &source_computer_id
        })
        .bind(&destination_path)
        .bind(&name)
        .bind(size)
        .bind(status)
        .bind(skip_reason)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn handoff_is_terminal(pool: &PgPool, delegation_id: &str) -> Result<bool, sqlx::Error> {
    let pending: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM delegation_artifact_transfers
        WHERE delegation_id = $1 AND status IN ('pending', 'transferring')
        "#,
    )
    .bind(delegation_id)
    .fetch_one(pool)
    .await?;
    Ok(pending == 0)
}

pub async fn execute_pending_transfers_for_delegation(
    state: &AppState,
    delegation_id: &str,
) -> Result<(), String> {
    let meta = sqlx::query(
        r#"
        SELECT d.owner_id, d.source_bot_id, sb.computer_id AS destination_computer_id
        FROM bot_delegations d
        JOIN bots sb ON sb.id = d.source_bot_id
        WHERE d.id = $1
        "#,
    )
    .bind(delegation_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    let Some(meta) = meta else {
        return Ok(());
    };

    let owner_id: String = meta.get("owner_id");
    let destination_computer_id: String = meta.get("destination_computer_id");

    let pending_ids: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT id FROM delegation_artifact_transfers
        WHERE delegation_id = $1 AND status = 'pending'
        ORDER BY created_at ASC, name ASC
        "#,
    )
    .bind(delegation_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    for transfer_id in pending_ids {
        if let Err(err) =
            execute_one_transfer(state, &owner_id, &destination_computer_id, &transfer_id).await
        {
            tracing::warn!(
                delegation_id = %delegation_id,
                transfer_id = %transfer_id,
                error = %err,
                "artifact transfer failed"
            );
        }
    }
    Ok(())
}

async fn execute_one_transfer(
    state: &AppState,
    owner_id: &str,
    destination_computer_id: &str,
    transfer_id: &str,
) -> Result<(), String> {
    let claimed = sqlx::query(
        r#"
        UPDATE delegation_artifact_transfers
        SET status = 'transferring', started_at = COALESCE(started_at, NOW())
        WHERE id = $1 AND status = 'pending'
        RETURNING delegation_id, source_result_id, destination_path, name, size
        "#,
    )
    .bind(transfer_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    let Some(row) = claimed else {
        return Ok(());
    };

    let outcome = perform_claimed_transfer(
        state,
        owner_id,
        destination_computer_id,
        &row.get::<String, _>("delegation_id"),
        row.get::<Uuid, _>("source_result_id"),
        &row.get::<String, _>("destination_path"),
        row.get::<i64, _>("size"),
    )
    .await;
    match outcome {
        Ok(sha256) => mark_transfer_completed(&state.pool, transfer_id, &sha256).await,
        Err((code, message)) => {
            mark_transfer_failed(&state.pool, transfer_id, code, &message).await
        }
    }
}

async fn perform_claimed_transfer(
    state: &AppState,
    owner_id: &str,
    destination_computer_id: &str,
    delegation_id: &str,
    source_result_id: Uuid,
    destination_path: &str,
    expected_size: i64,
) -> Result<String, (&'static str, String)> {
    let authorized = verify_transfer_authorized(&state.pool, delegation_id, source_result_id)
        .await
        .map_err(|e| ("unauthorized", e.to_string()))?;
    if !authorized {
        return Err(("unauthorized", "Artifact transfer is not authorized".into()));
    }

    let content: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT content FROM work_results WHERE id = $1")
            .bind(source_result_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| ("missing_result", e.to_string()))?;
    let Some(content) = content else {
        return Err((
            "missing_result",
            "Source artifact is no longer available".into(),
        ));
    };
    if content.len() as i64 != expected_size {
        return Err(("size_mismatch", "Artifact size changed".into()));
    }

    let sha256 = hex_sha256(&content);
    write_artifact_bytes(
        state,
        owner_id,
        destination_computer_id,
        destination_path,
        &content,
        &sha256,
    )
    .await?;
    Ok(sha256)
}

fn destination_connect_failure(err: ApiError) -> (&'static str, String) {
    match err {
        ApiError::NotFound => (
            "destination_unavailable",
            "The destination computer is no longer available".into(),
        ),
        ApiError::Validation(message) => ("destination_unavailable", message),
        other => ("destination_unavailable", other.to_string()),
    }
}

async fn write_artifact_bytes(
    state: &AppState,
    owner_id: &str,
    destination_computer_id: &str,
    destination_path: &str,
    content: &[u8],
    sha256: &str,
) -> Result<(), (&'static str, String)> {
    let computer = state
        .computer_registry
        .connect_agent_computer(
            &state.config,
            &state.pool,
            owner_id,
            destination_computer_id,
            &state.local_mac_sessions,
            false,
        )
        .await
        .map_err(destination_connect_failure)?;

    if let Ok(existing) = computer.read_file(destination_path).await {
        if existing.len() == content.len() && hex_sha256(&existing) == sha256 {
            bump_workspace_revision(computer.as_ref());
            return Ok(());
        }
        return Err((
            "destination_conflict",
            "Destination file exists with different content".into(),
        ));
    }

    computer
        .ensure_ready()
        .await
        .map_err(|err| (err.code(), err.to_string()))?;
    computer
        .write_file(destination_path, content)
        .await
        .map_err(|err| (err.code(), err.to_string()))?;
    let written = computer
        .read_file(destination_path)
        .await
        .map_err(|err| (err.code(), err.to_string()))?;
    if written.len() != content.len() || hex_sha256(&written) != sha256 {
        return Err(("verify_failed", "Could not verify written artifact".into()));
    }
    bump_workspace_revision(computer.as_ref());
    Ok(())
}

async fn verify_transfer_authorized(
    pool: &PgPool,
    delegation_id: &str,
    source_result_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let ok: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
          SELECT 1
          FROM delegation_artifact_transfers t
          JOIN bot_delegations d ON d.id = t.delegation_id
          JOIN agent_runs tr ON tr.id = d.target_run_id
          JOIN work_results wr ON wr.id = t.source_result_id AND wr.run_id = tr.id
          JOIN agent_runs sr ON sr.id = d.source_run_id
          WHERE t.delegation_id = $1
            AND t.source_result_id = $2
            AND d.owner_id = tr.owner_id
            AND d.owner_id = sr.owner_id
        )
        "#,
    )
    .bind(delegation_id)
    .bind(source_result_id)
    .fetch_one(pool)
    .await?;
    Ok(ok)
}

async fn mark_transfer_completed(
    pool: &PgPool,
    transfer_id: &str,
    sha256: &str,
) -> Result<(), String> {
    sqlx::query(
        r#"
        UPDATE delegation_artifact_transfers
        SET status = 'completed',
            sha256 = $2,
            finished_at = NOW(),
            error_code = NULL,
            error_message = NULL
        WHERE id = $1
        "#,
    )
    .bind(transfer_id)
    .bind(sha256)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

async fn mark_transfer_failed(
    pool: &PgPool,
    transfer_id: &str,
    code: &str,
    message: &str,
) -> Result<(), String> {
    sqlx::query(
        r#"
        UPDATE delegation_artifact_transfers
        SET status = 'failed',
            error_code = $2,
            error_message = $3,
            finished_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(transfer_id)
    .bind(code)
    .bind(message)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub async fn load_artifact_context_lines<'e, E>(
    executor: E,
    delegation_id: &str,
) -> Result<Vec<ArtifactContextLine>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let rows = sqlx::query(
        r#"
        SELECT name, destination_path, status, error_code, error_message, skip_reason
        FROM delegation_artifact_transfers
        WHERE delegation_id = $1
        ORDER BY created_at ASC, name ASC
        LIMIT 20
        "#,
    )
    .bind(delegation_id)
    .fetch_all(executor)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let status: String = row.get("status");
            let path: String = row.get("destination_path");
            let error_code: Option<String> = row.get("error_code");
            let error_message: Option<String> = row.get("error_message");
            let error = if status == "failed" {
                Some(
                    error_message
                        .or(error_code)
                        .unwrap_or_else(|| "transfer failed".into()),
                )
            } else {
                None
            };
            ArtifactContextLine {
                name: row.get("name"),
                path: if status == "completed" || status == "skipped" {
                    Some(path)
                } else {
                    None
                },
                transfer_status: status,
                error,
            }
        })
        .collect())
}

pub async fn reclaim_abandoned_transfers(pool: &PgPool) -> Result<u64, sqlx::Error> {
    reclaim_abandoned_transfers_older_than(pool, ARTIFACT_TRANSFER_LEASE).await
}

pub async fn reclaim_abandoned_transfers_older_than(
    pool: &PgPool,
    age: Duration,
) -> Result<u64, sqlx::Error> {
    let age_secs = i64::try_from(age.as_secs()).unwrap_or(i64::MAX);
    let exhausted = sqlx::query(
        r#"
        UPDATE delegation_artifact_transfers
        SET status = 'failed',
            error_code = 'transfer_interrupted',
            error_message = 'Artifact transfer was interrupted',
            finished_at = NOW()
        WHERE status = 'transferring'
          AND error_code = 'reclaimed'
          AND started_at < NOW() - ($1 * INTERVAL '1 second')
        "#,
    )
    .bind(age_secs)
    .execute(pool)
    .await?
    .rows_affected();

    let retried = sqlx::query(
        r#"
        UPDATE delegation_artifact_transfers
        SET status = 'pending',
            error_code = 'reclaimed',
            started_at = NULL
        WHERE status = 'transferring'
          AND COALESCE(error_code, '') <> 'reclaimed'
          AND started_at < NOW() - ($1 * INTERVAL '1 second')
        "#,
    )
    .bind(age_secs)
    .execute(pool)
    .await?
    .rows_affected();

    Ok(exhausted + retried)
}

pub async fn reconcile_artifact_handoffs(state: &AppState) -> Result<(), sqlx::Error> {
    reclaim_abandoned_transfers(&state.pool).await?;
    let delegation_ids: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT d.id
        FROM bot_delegations d
        JOIN agent_runs tr ON tr.id = d.target_run_id
        WHERE d.return_policy = 'resume_source'
          AND tr.status = 'completed'
          AND tr.results_status IN ('complete', 'partial', 'failed')
          AND d.status IN ('completed', 'failed', 'cancelled')
          AND d.source_resume_run_id IS NULL
          AND COALESCE(d.resume_status, '') NOT IN ('skipped', 'running', 'completed', 'failed')
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    for delegation_id in delegation_ids {
        plan_transfers_for_delegation(&state.pool, &delegation_id).await?;
        execute_pending_transfers_for_delegation(state, &delegation_id)
            .await
            .map_err(|e| sqlx::Error::Protocol(e))?;
        crate::run_lifecycle::try_admit_delegation_return(&state.pool, &delegation_id).await?;
    }
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DelegationArtifactSummary {
    pub result_id: String,
    pub name: String,
    pub size: i64,
    pub transfer_status: String,
    pub destination_path: Option<String>,
    pub error: Option<String>,
}

pub async fn list_artifacts_for_delegation(
    pool: &PgPool,
    owner: &str,
    delegation_id: &str,
) -> Result<Vec<DelegationArtifactSummary>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT t.source_result_id, t.name, t.size, t.status, t.destination_path,
               t.error_code, t.error_message
        FROM delegation_artifact_transfers t
        JOIN bot_delegations d ON d.id = t.delegation_id
        WHERE d.owner_id = $1 AND d.id = $2
        ORDER BY t.created_at ASC, t.name ASC
        LIMIT 20
        "#,
    )
    .bind(owner)
    .bind(delegation_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let status: String = row.get("status");
            let error_code: Option<String> = row.get("error_code");
            let error_message: Option<String> = row.get("error_message");
            let destination_path: String = row.get("destination_path");
            DelegationArtifactSummary {
                result_id: row.get("source_result_id"),
                name: row.get("name"),
                size: row.get("size"),
                transfer_status: status.clone(),
                destination_path: if status == "completed" || status == "skipped" {
                    Some(destination_path)
                } else {
                    None
                },
                error: if status == "failed" {
                    Some(
                        error_message
                            .or(error_code)
                            .unwrap_or_else(|| "failed".into()),
                    )
                } else {
                    None
                },
            }
        })
        .collect())
}

pub async fn delegation_return_ready_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    delegation_id: &str,
) -> Result<bool, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT tr.status AS target_run_status, tr.results_status
        FROM bot_delegations d
        JOIN agent_runs tr ON tr.id = d.target_run_id
        WHERE d.id = $1
        "#,
    )
    .bind(delegation_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some(row) = row else {
        return Ok(false);
    };

    let target_run_status: String = row.get("target_run_status");
    if target_run_status == "completed" {
        let results_status: Option<String> = row.get("results_status");
        let results_status = results_status.unwrap_or_else(|| "pending".into());
        if !crate::result_finalization::is_results_status_terminal(&results_status) {
            return Ok(false);
        }
        let pending: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM delegation_artifact_transfers
            WHERE delegation_id = $1 AND status IN ('pending', 'transferring')
            "#,
        )
        .bind(delegation_id)
        .fetch_one(&mut **tx)
        .await?;
        return Ok(pending == 0);
    }

    Ok(crate::run_lifecycle::is_terminal_run_status(
        &target_run_status,
    ))
}
