//! Durable bot-to-bot delegations and lifecycle sync.

use serde_json::json;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use agent_core::{
    BotTeammateSummary, CollaborationContext, CollaborationError, DelegationEnqueueResult,
    MAX_CHILD_DELEGATIONS_PER_ROOT, MAX_DELEGATION_CONTEXT_CHARS, MAX_DELEGATION_DEPTH,
    MAX_DELEGATION_INSTRUCTION_CHARS,
};

use crate::error::ApiError;
use crate::work;

fn db_error(error: sqlx::Error) -> CollaborationError {
    CollaborationError::Internal(error.to_string())
}

pub fn format_delegated_user_message(
    source_bot_name: &str,
    source_run_id: &str,
    instruction: &str,
    context: Option<&str>,
) -> String {
    let mut message = format!(
        "Delegated by {source_bot_name} from run {source_run_id}:\n{instruction}"
    );
    if let Some(ctx) = context.filter(|c| !c.trim().is_empty()) {
        message.push_str("\n\nAdditional context:\n");
        message.push_str(ctx.trim());
    }
    message
}

pub fn format_delegation_return_user_message(
    target_bot_name: &str,
    instruction: &str,
    target_status: &str,
    target_result: &str,
    target_run_id: &str,
    artifact_lines: &[String],
    shared_computer: bool,
    interruption_detail: Option<&str>,
) -> String {
    let mut message = format!(
        "Delegated work returned from {target_bot_name}.\n\n\
Original delegated instruction:\n{instruction}\n\n\
Recipient status:\n{target_status}\n\n\
Recipient result:\n{target_result}\n\n\
Target run:\n{target_run_id}\n"
    );
    if let Some(detail) = interruption_detail.filter(|d| !d.is_empty()) {
        message.push_str("\nInterruption:\n");
        message.push_str(detail);
        message.push('\n');
    }
    if !artifact_lines.is_empty() {
        message.push_str("\nTarget artifacts:\n");
        for line in artifact_lines {
            message.push_str("- ");
            message.push_str(line);
            message.push('\n');
        }
    }
    if shared_computer {
        message.push_str(
            "\nSource and target Bots share the same computer; workspace files from the target run may still be available under /workspace.\n",
        );
    } else {
        message.push_str(
            "\nSource and target Bots use different computers; target workspace files are not locally accessible on your computer.\n",
        );
    }
    message.push_str(
        "\nContinue the original task using these findings.\n\
Do not claim access to files that are on another computer unless they are actually accessible.",
    );
    message
}

pub async fn list_teammates(
    pool: &PgPool,
    owner: &str,
    source_bot_id: &str,
) -> Result<Vec<BotTeammateSummary>, CollaborationError> {
    let rows = sqlx::query(
        r#"
        SELECT b.id, b.name, b.system_prompt,
               EXISTS(
                 SELECT 1 FROM sandboxes s
                 WHERE s.id = b.computer_id AND s.owner_id = b.owner_id AND s.state <> 'archived'
               ) AS available
        FROM bots b
        WHERE b.owner_id = $1
        ORDER BY b.name ASC, b.id ASC
        "#,
    )
    .bind(owner)
    .fetch_all(pool)
    .await
    .map_err(db_error)?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let id: String = row.get("id");
            let is_self = id == source_bot_id;
            let name: String = row.get("name");
            let prompt: String = row.get("system_prompt");
            let available: bool = row.get("available");
            let role_summary = summarize_role(&prompt);
            BotTeammateSummary {
                id,
                name,
                role_summary,
                available,
                is_self,
            }
        })
        .collect())
}

fn summarize_role(prompt: &str) -> String {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return "General assistant".into();
    }
    let first_line = trimmed.lines().next().unwrap_or(trimmed);
    if first_line.len() > 120 {
        format!("{}…", &first_line[..120])
    } else {
        first_line.to_string()
    }
}

pub async fn create_delegation(
    pool: &PgPool,
    ctx: &CollaborationContext,
    target_bot_id: &str,
    instruction: &str,
    context: Option<&str>,
    return_policy: &str,
) -> Result<DelegationEnqueueResult, CollaborationError> {
    if return_policy != "none" && return_policy != "resume_source" {
        return Err(CollaborationError::Validation(
            "return_policy must be none or resume_source".into(),
        ));
    }
    if target_bot_id == ctx.source_bot_id {
        return Err(CollaborationError::Validation(
            "Cannot delegate work to yourself".into(),
        ));
    }
    let instruction = instruction.trim();
    if instruction.is_empty() || instruction.len() > MAX_DELEGATION_INSTRUCTION_CHARS {
        return Err(CollaborationError::Validation(
            format!(
                "instruction must be between 1 and {} characters",
                MAX_DELEGATION_INSTRUCTION_CHARS
            ),
        ));
    }
    if let Some(ctx_text) = context {
        if ctx_text.len() > MAX_DELEGATION_CONTEXT_CHARS {
            return Err(CollaborationError::Validation(
                format!(
                    "context must be at most {} characters",
                    MAX_DELEGATION_CONTEXT_CHARS
                ),
            ));
        }
    }
    if ctx.tool_invocation_id.is_empty() || ctx.tool_invocation_id.len() > 200 {
        return Err(CollaborationError::Validation(
            "tool invocation id is missing or too long".into(),
        ));
    }

    let mut tx = pool.begin().await.map_err(db_error)?;

    if let Some(result) =
        load_delegation_by_invocation(&mut tx, &ctx.source_run_id, &ctx.tool_invocation_id, &ctx.owner_id)
            .await?
    {
        tx.commit().await.map_err(db_error)?;
        return Ok(result);
    }

    let source_row = sqlx::query(
        "SELECT owner_id, bot_id FROM agent_runs WHERE id = $1 AND owner_id = $2",
    )
    .bind(&ctx.source_run_id)
    .bind(&ctx.owner_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(db_error)?
    .ok_or(CollaborationError::NotFound)?;

    if source_row.get::<String, _>("bot_id") != ctx.source_bot_id {
        return Err(CollaborationError::Internal(
            "source run bot mismatch".into(),
        ));
    }

    let (root_run_id, parent_delegation_id, depth) =
        resolve_delegation_chain(&mut tx, &ctx.source_run_id).await?;

    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(format!("delegation-root:{root_run_id}"))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

    if let Some(result) =
        load_delegation_by_invocation(&mut tx, &ctx.source_run_id, &ctx.tool_invocation_id, &ctx.owner_id)
            .await?
    {
        tx.commit().await.map_err(db_error)?;
        return Ok(result);
    }

    if depth >= MAX_DELEGATION_DEPTH {
        return Err(CollaborationError::LimitExceeded(format!(
            "Delegation depth limit ({}) exceeded",
            MAX_DELEGATION_DEPTH
        )));
    }

    let child_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM bot_delegations WHERE root_run_id = $1",
    )
    .bind(&root_run_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(db_error)?;
    if child_count >= MAX_CHILD_DELEGATIONS_PER_ROOT {
        return Err(CollaborationError::LimitExceeded(format!(
            "At most {} delegations per root assignment",
            MAX_CHILD_DELEGATIONS_PER_ROOT
        )));
    }

    let target = sqlx::query(
        "SELECT b.id, b.name FROM bots b JOIN sandboxes s ON s.id = b.computer_id AND s.owner_id = b.owner_id WHERE b.id = $1 AND b.owner_id = $2 AND s.state <> 'archived'",
    )
    .bind(target_bot_id)
    .bind(&ctx.owner_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(db_error)?
    .ok_or(CollaborationError::NotFound)?;

    let target_bot_name: String = target.get("name");
    let delegation_id = Uuid::new_v4().to_string();
    let source_bot_name: String =
        sqlx::query_scalar("SELECT name FROM bots WHERE id = $1 AND owner_id = $2")
            .bind(&ctx.source_bot_id)
            .bind(&ctx.owner_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(db_error)?;

    let user_message = format_delegated_user_message(
        &source_bot_name,
        &ctx.source_run_id,
        instruction,
        context,
    );

    let target_request_id = format!("delegation-{delegation_id}");

    let delegation_insert = sqlx::query(
        r#"
        INSERT INTO bot_delegations (
            id, owner_id, source_bot_id, target_bot_id, source_run_id, source_conversation_id,
            source_request_id, root_run_id, parent_delegation_id, depth, instruction, context,
            status, tool_invocation_id, return_policy
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,'queued',$13,$14)
        "#,
    )
    .bind(&delegation_id)
    .bind(&ctx.owner_id)
    .bind(&ctx.source_bot_id)
    .bind(target_bot_id)
    .bind(&ctx.source_run_id)
    .bind(&ctx.source_conversation_id)
    .bind(&ctx.source_request_id)
    .bind(&root_run_id)
    .bind(&parent_delegation_id)
    .bind(depth + 1)
    .bind(instruction)
    .bind(context)
    .bind(&ctx.tool_invocation_id)
    .bind(return_policy)
    .execute(&mut *tx)
    .await;

    if let Err(err) = delegation_insert {
        if is_unique_violation(&err) {
            if let Some(result) = load_delegation_by_invocation(
                &mut tx,
                &ctx.source_run_id,
                &ctx.tool_invocation_id,
                &ctx.owner_id,
            )
            .await?
            {
                tx.commit().await.map_err(db_error)?;
                return Ok(result);
            }
        }
        return Err(db_error(err));
    }

    let records = work::enqueue_delegated_in_transaction(
        &mut tx,
        &ctx.owner_id,
        &target_request_id,
        target_bot_id,
        None,
        &user_message,
        &delegation_id,
    )
    .await
    .map_err(map_work_error)?;

    sqlx::query(
        "UPDATE bot_delegations SET target_run_id = $2, target_conversation_id = $3 WHERE id = $1",
    )
    .bind(&delegation_id)
    .bind(&records.run_id)
    .bind(&records.conversation_id)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;

    let queued_payload = json!({
        "delegationId": delegation_id,
        "targetBotId": target_bot_id,
        "targetBotName": target_bot_name,
        "targetRunId": records.run_id,
        "status": "queued"
    });
    sqlx::query(
        "INSERT INTO run_events (request_id, event_type, payload_json) VALUES ($1, 'bot_delegation_queued', $2)",
    )
    .bind(&ctx.source_request_id)
    .bind(&queued_payload)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;

    tx.commit().await.map_err(db_error)?;

    Ok(DelegationEnqueueResult {
        delegation_id,
        target_bot_id: target_bot_id.to_string(),
        target_bot_name,
        target_run_id: records.run_id,
        status: "queued".into(),
    })
}

async fn resolve_delegation_chain(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    source_run_id: &str,
) -> Result<(String, Option<String>, i32), CollaborationError> {
    if let Some(delegation_id) = sqlx::query_scalar::<_, String>(
        "SELECT delegation_id FROM work_queue WHERE run_id = $1 AND provenance_kind = 'delegation_return'",
    )
    .bind(source_run_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    {
        let row = sqlx::query(
            "SELECT root_run_id, parent_delegation_id, depth FROM bot_delegations WHERE id = $1",
        )
        .bind(&delegation_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(db_error)?
        .ok_or(CollaborationError::Internal(
            "delegation return missing parent row".into(),
        ))?;
        let depth: i32 = row.get("depth");
        return Ok((
            row.get("root_run_id"),
            row.get("parent_delegation_id"),
            depth.saturating_sub(1),
        ));
    }

    let parent = sqlx::query(
        "SELECT id, root_run_id, depth FROM bot_delegations WHERE target_run_id = $1",
    )
    .bind(source_run_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?;

    if let Some(row) = parent {
        Ok((
            row.get("root_run_id"),
            Some(row.get("id")),
            row.get::<i32, _>("depth"),
        ))
    } else {
        Ok((source_run_id.to_string(), None, 0))
    }
}

fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(
        err,
        sqlx::Error::Database(db) if db.code().as_deref() == Some("23505")
    )
}

async fn load_delegation_by_invocation(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    source_run_id: &str,
    tool_invocation_id: &str,
    owner_id: &str,
) -> Result<Option<DelegationEnqueueResult>, CollaborationError> {
    let existing = sqlx::query(
        "SELECT id, target_bot_id, target_run_id, status FROM bot_delegations WHERE source_run_id = $1 AND tool_invocation_id = $2",
    )
    .bind(source_run_id)
    .bind(tool_invocation_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?;

    let Some(existing) = existing else {
        return Ok(None);
    };

    let delegation_id: String = existing.get("id");
    let target_run_id: Option<String> = existing.get("target_run_id");
    let status: String = existing.get("status");
    let target_bot_id_stored: String = existing.get("target_bot_id");
    let target_name: String =
        sqlx::query_scalar("SELECT name FROM bots WHERE id = $1 AND owner_id = $2")
            .bind(&target_bot_id_stored)
            .bind(owner_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(db_error)?
            .unwrap_or_else(|| "Bot".into());

    Ok(Some(DelegationEnqueueResult {
        delegation_id,
        target_bot_id: target_bot_id_stored,
        target_bot_name: target_name,
        target_run_id: target_run_id.unwrap_or_default(),
        status,
    }))
}

fn map_work_error(err: ApiError) -> CollaborationError {
    match err {
        ApiError::NotFound => CollaborationError::NotFound,
        ApiError::Validation(m) => CollaborationError::Validation(m),
        ApiError::Conflict(m) => CollaborationError::Conflict(m),
        ApiError::TooManyRequests => CollaborationError::LimitExceeded(
            "Too much active work; try again later".into(),
        ),
        ApiError::Internal(m) => CollaborationError::Internal(m),
        ApiError::Unauthorized => CollaborationError::Internal("unauthorized".into()),
    }
}

pub async fn on_target_run_claimed(pool: &PgPool, run_id: &str) -> Result<(), sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, source_request_id, status FROM bot_delegations WHERE target_run_id = $1",
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(());
    };
    let status: String = row.get("status");
    if status != "queued" {
        return Ok(());
    }
    let delegation_id: String = row.get("id");
    let source_request_id: String = row.get("source_request_id");

    sqlx::query(
        "UPDATE bot_delegations SET status = 'running', started_at = COALESCE(started_at, NOW()) WHERE id = $1",
    )
    .bind(&delegation_id)
    .execute(pool)
    .await?;

    let payload = json!({
        "delegationId": delegation_id,
        "targetRunId": run_id,
        "status": "running"
    });
    sqlx::query(
        "INSERT INTO run_events (request_id, event_type, payload_json) VALUES ($1, 'bot_delegation_running', $2)",
    )
    .bind(&source_request_id)
    .bind(&payload)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn sync_target_run_terminal(
    pool: &PgPool,
    request_id: &str,
    status: &str,
    error_code: Option<&str>,
) -> Result<(), sqlx::Error> {
    crate::run_lifecycle::synchronize_run_terminal_for_request(pool, request_id, status, error_code)
        .await
}

pub async fn reconcile_pending_delegation_returns(pool: &PgPool) -> Result<(), sqlx::Error> {
    crate::run_lifecycle::reconcile_collaboration_lifecycle(pool).await
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DelegationDetail {
    pub id: String,
    pub status: String,
    pub instruction: String,
    pub context: Option<String>,
    pub depth: i32,
    pub source_bot_id: String,
    pub source_bot_name: String,
    pub target_bot_id: String,
    pub target_bot_name: String,
    pub target_bot_avatar_id: String,
    pub source_run_id: String,
    pub target_run_id: Option<String>,
    pub target_run_status: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub return_policy: String,
    pub source_resume_run_id: Option<String>,
    pub resume_status: Option<String>,
    pub resume_error: Option<String>,
}

pub async fn list_for_run(
    pool: &PgPool,
    owner: &str,
    run_id: &str,
) -> Result<Vec<DelegationDetail>, ApiError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM agent_runs WHERE id = $1 AND owner_id = $2)",
    )
    .bind(run_id)
    .bind(owner)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    if !exists {
        return Err(ApiError::NotFound);
    }

    let rows = sqlx::query(
        r#"
        SELECT d.*,
               sb.name AS source_bot_name,
               tb.name AS target_bot_name,
               tb.avatar_id AS target_bot_avatar_id,
               tr.status AS target_run_status
        FROM bot_delegations d
        JOIN bots sb ON sb.id = d.source_bot_id
        JOIN bots tb ON tb.id = d.target_bot_id
        LEFT JOIN agent_runs tr ON tr.id = d.target_run_id
        WHERE d.owner_id = $1 AND d.source_run_id = $2
        ORDER BY d.created_at ASC
        "#,
    )
    .bind(owner)
    .bind(run_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(rows.into_iter().map(map_delegation_row).collect())
}

pub async fn get_delegation(
    pool: &PgPool,
    owner: &str,
    delegation_id: &str,
) -> Result<DelegationDetail, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT d.*,
               sb.name AS source_bot_name,
               tb.name AS target_bot_name,
               tb.avatar_id AS target_bot_avatar_id,
               tr.status AS target_run_status
        FROM bot_delegations d
        JOIN bots sb ON sb.id = d.source_bot_id
        JOIN bots tb ON tb.id = d.target_bot_id
        LEFT JOIN agent_runs tr ON tr.id = d.target_run_id
        WHERE d.owner_id = $1 AND d.id = $2
        "#,
    )
    .bind(owner)
    .bind(delegation_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or(ApiError::NotFound)?;

    Ok(map_delegation_row(row))
}

fn map_delegation_row(row: sqlx::postgres::PgRow) -> DelegationDetail {
    DelegationDetail {
        id: row.get("id"),
        status: row.get("status"),
        instruction: row.get("instruction"),
        context: row.get("context"),
        depth: row.get("depth"),
        source_bot_id: row.get("source_bot_id"),
        source_bot_name: row.get("source_bot_name"),
        target_bot_id: row.get("target_bot_id"),
        target_bot_name: row.get("target_bot_name"),
        target_bot_avatar_id: row.get("target_bot_avatar_id"),
        source_run_id: row.get("source_run_id"),
        target_run_id: row.get("target_run_id"),
        target_run_status: row.get("target_run_status"),
        created_at: row.get("created_at"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        error_code: row.get("error_code"),
        error_message: row.get("error_message"),
        return_policy: row.get("return_policy"),
        source_resume_run_id: row.get("source_resume_run_id"),
        resume_status: row.get("resume_status"),
        resume_error: row.get("resume_error"),
    }
}
