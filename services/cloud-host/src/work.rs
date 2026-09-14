//! Durable product work admission and dispatch. Side effects are never replayed after interruption.
use sqlx::{PgPool, Row};
use uuid::Uuid;

use agent_core::{compose_runtime_instruction_snapshot, RuntimeIdentityInput};

use crate::{
    conversation::get_or_create_primary_conversation_id_in_tx,
    db::queries::BootstrapRunRecords,
    error::ApiError,
    runner::RunExecutionInput,
};

fn db_error(error: sqlx::Error) -> ApiError {
    ApiError::Internal(error.to_string())
}

pub async fn enqueue(
    pool: &PgPool,
    owner: &str,
    request_id: &str,
    bot_id: &str,
    conversation_id: Option<&str>,
    message: &str,
) -> Result<BootstrapRunRecords, ApiError> {
    let mut tx = pool.begin().await.map_err(db_error)?;
    let records =
        enqueue_in_transaction(&mut tx, owner, request_id, bot_id, conversation_id, message)
            .await?;
    tx.commit().await.map_err(db_error)?;
    Ok(records)
}

pub async fn enqueue_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &str,
    request_id: &str,
    bot_id: &str,
    conversation_id: Option<&str>,
    message: &str,
) -> Result<BootstrapRunRecords, ApiError> {
    if message.trim().is_empty() || message.len() > 100_000 {
        return Err(ApiError::Validation(
            "Work must contain between 1 and 100,000 bytes".into(),
        ));
    }
    if request_id.len() > 200 {
        return Err(ApiError::Validation("Idempotency key is too long".into()));
    }
    // Serialize admission per owner as well as idempotency keys, including queue limits.
    for key in [
        format!("work-owner:{owner}"),
        format!("work-request:{request_id}"),
    ] {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
            .bind(key)
            .execute(&mut **tx)
            .await
            .map_err(db_error)?;
    }
    if let Some(row) = sqlx::query("SELECT r.*, q.instructions, q.user_message FROM agent_runs r LEFT JOIN work_queue q ON q.run_id = r.id WHERE request_id = $1")
        .bind(request_id).fetch_optional(&mut **tx).await.map_err(db_error)? {
        if row.get::<String, _>("owner_id") != owner { return Err(ApiError::NotFound); }
        if row.get::<String, _>("bot_id") != bot_id || row.get::<Option<String>, _>("user_message").as_deref() != Some(message.trim())
            || conversation_id.is_some_and(|id| id != row.get::<String, _>("conversation_id")) {
            return Err(ApiError::Conflict("This request key was already used for different work".into()));
        }
        return Ok(BootstrapRunRecords {
            run_id: row.get("id"), request_id: request_id.into(), conversation_id: row.get("conversation_id"),
            assistant_message_id: row.get("assistant_message_id"), computer_id: row.get("computer_id"),
            model: row.get("model"), instructions: row.get("instructions"), is_new_run: false,
        });
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agent_runs WHERE owner_id = $1 AND status IN ('queued', 'running')",
    )
    .bind(owner)
    .fetch_one(&mut **tx)
    .await
    .map_err(db_error)?;
    if count >= 100 {
        return Err(ApiError::TooManyRequests);
    }
    let bot = sqlx::query("SELECT b.name, b.model, b.system_prompt, b.engine_preference, b.computer_id FROM bots b JOIN sandboxes s ON s.id = b.computer_id AND s.owner_id = b.owner_id WHERE b.id = $1 AND b.owner_id = $2 AND s.state <> 'archived' FOR SHARE OF b, s")
        .bind(bot_id).bind(owner).fetch_optional(&mut **tx).await.map_err(db_error)?
        .ok_or_else(|| ApiError::Validation("Choose a bot with an available computer".into()))?;
    let model: String = bot.get("model");
    let bot_name: String = bot.get("name");
    let system_prompt: String = bot.get("system_prompt");
    let context: Option<String> =
        sqlx::query_scalar("SELECT content FROM bot_context WHERE bot_id = $1")
            .bind(bot_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(db_error)?;
    let instructions = compose_runtime_instruction_snapshot(&RuntimeIdentityInput {
        bot_name,
        role_instructions: system_prompt,
        saved_context: context.filter(|value| !value.is_empty()),
    });
    let computer_id: String = bot.get("computer_id");
    let engine: String = bot.get("engine_preference");
    let conversation_id = if let Some(id) = conversation_id {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM conversations WHERE id = $1 AND bot_id = $2 AND owner_id = $3)")
            .bind(id).bind(bot_id).bind(owner).fetch_one(&mut **tx).await.map_err(db_error)?;
        if !exists {
            return Err(ApiError::NotFound);
        }
        id.to_string()
    } else {
        get_or_create_primary_conversation_id_in_tx(tx, owner, bot_id).await?
    };
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(format!("conversation-seq:{conversation_id}"))
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
    let sequence: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sequence), 0) + 1 FROM messages WHERE conversation_id = $1",
    )
    .bind(&conversation_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(db_error)?;
    let assistant_message_id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO messages (id, conversation_id, role, body, status, sequence, model) VALUES ($1, $2, 'user', $3, 'complete', $4, $5), ($6, $2, 'assistant', '', 'pending', $4 + 1, $5)")
        .bind(Uuid::new_v4().to_string()).bind(&conversation_id).bind(message.trim()).bind(sequence).bind(&model).bind(&assistant_message_id)
        .execute(&mut **tx).await.map_err(db_error)?;
    let run_id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO agent_runs (id, owner_id, request_id, bot_id, conversation_id, computer_id, model, status, assistant_message_id) VALUES ($1,$2,$3,$4,$5,$6,$7,'queued',$8)")
        .bind(&run_id).bind(owner).bind(request_id).bind(bot_id).bind(&conversation_id).bind(&computer_id).bind(&model).bind(&assistant_message_id)
        .execute(&mut **tx).await.map_err(db_error)?;
    sqlx::query("INSERT INTO work_queue (run_id, user_message, instructions, engine_preference) VALUES ($1,$2,$3,$4)")
        .bind(&run_id).bind(message.trim()).bind(&instructions).bind(&engine).execute(&mut **tx).await.map_err(db_error)?;
    sqlx::query("INSERT INTO run_events (request_id, event_type, payload_json) VALUES ($1, 'queued', $2)")
        .bind(request_id).bind(serde_json::json!({"status":"queued", "detail":"Work saved. Waiting for an available computer."}))
        .execute(&mut **tx).await.map_err(db_error)?;
    sqlx::query("UPDATE conversations SET updated_at = NOW() WHERE id = $1")
        .bind(&conversation_id)
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
    Ok(BootstrapRunRecords {
        run_id,
        request_id: request_id.into(),
        conversation_id,
        assistant_message_id,
        computer_id,
        model,
        instructions,
        is_new_run: true,
    })
}

/// Atomic claim serializes all dispatchers, and only one work item uses a computer or bot at a time.
pub async fn claim_next(pool: &PgPool) -> Result<Option<RunExecutionInput>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(18493721)")
        .execute(&mut *tx)
        .await?;
    let row = sqlx::query("SELECT r.*, q.user_message, q.instructions, q.engine_preference FROM agent_runs r JOIN work_queue q ON q.run_id = r.id WHERE r.status = 'queued' AND NOT EXISTS(SELECT 1 FROM agent_runs active WHERE (active.status = 'running' OR (active.started_at IS NOT NULL AND active.execution_released_at IS NULL)) AND (active.computer_id = r.computer_id OR active.bot_id = r.bot_id)) ORDER BY r.created_at, r.id LIMIT 1 FOR UPDATE OF r SKIP LOCKED")
        .fetch_optional(&mut *tx).await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let run_id: String = row.get("id");
    sqlx::query("UPDATE agent_runs SET status = 'running', started_at = NOW(), updated_at = NOW() WHERE id = $1")
        .bind(&run_id).execute(&mut *tx).await?;
    let input = RunExecutionInput {
        records: BootstrapRunRecords {
            run_id,
            request_id: row.get("request_id"),
            conversation_id: row.get("conversation_id"),
            assistant_message_id: row.get("assistant_message_id"),
            computer_id: row.get("computer_id"),
            model: row.get("model"),
            instructions: row.get("instructions"),
            is_new_run: true,
        },
        bot_id: row.get("bot_id"),
        user_message: row.get("user_message"),
        engine_mode: crate::run_engine_select::parse_run_engine_mode(row.get("engine_preference"))
            .ok(),
    };
    tx.commit().await?;
    Ok(Some(input))
}

pub async fn request_cancel(pool: &PgPool, owner: &str, run_id: &str) -> Result<(), ApiError> {
    let mut tx = pool.begin().await.map_err(db_error)?;
    let row = sqlx::query("SELECT status, request_id, assistant_message_id FROM agent_runs WHERE id = $1 AND owner_id = $2 FOR UPDATE")
        .bind(run_id).bind(owner).fetch_optional(&mut *tx).await.map_err(db_error)?.ok_or(ApiError::NotFound)?;
    let status: String = row.get("status");
    if status == "queued" {
        sqlx::query("UPDATE agent_runs SET status = 'cancelled', cancel_requested = TRUE, finished_at = NOW(), updated_at = NOW() WHERE id = $1")
            .bind(run_id).execute(&mut *tx).await.map_err(db_error)?;
        sqlx::query("UPDATE messages SET status = 'cancelled', updated_at = NOW() WHERE id = $1")
            .bind(row.get::<String, _>("assistant_message_id"))
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        sqlx::query("INSERT INTO run_events (request_id, event_type, payload_json) VALUES ($1, 'cancelled', $2)")
            .bind(row.get::<String,_>("request_id")).bind(serde_json::json!({"status":"cancelled"})).execute(&mut *tx).await.map_err(db_error)?;
    } else if status == "running" {
        sqlx::query("UPDATE agent_runs SET cancel_requested = TRUE WHERE id = $1")
            .bind(run_id)
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
    }
    tx.commit().await.map_err(db_error)?;
    Ok(())
}
