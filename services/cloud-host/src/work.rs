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
        crate::groups::assert_bot_may_use_conversation(tx, owner, bot_id, id).await?;
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
    let user_message_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO messages (id, conversation_id, role, body, status, sequence, model, author_kind)
        VALUES ($1, $2, 'user', $3, 'complete', $4, $5, 'human'),
               ($6, $2, 'assistant', '', 'pending', $4 + 1, $5, 'bot')
        "#,
    )
    .bind(&user_message_id)
    .bind(&conversation_id)
    .bind(message.trim())
    .bind(sequence)
    .bind(&model)
    .bind(&assistant_message_id)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    sqlx::query("UPDATE messages SET author_bot_id = $2 WHERE id = $1")
        .bind(&assistant_message_id)
        .bind(bot_id)
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
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

/// Enqueue a bot run against an existing group human message (no duplicate user transcript row).
pub async fn enqueue_from_group_message_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &str,
    request_id: &str,
    bot_id: &str,
    conversation_id: &str,
    source_message_id: &str,
    user_message: &str,
) -> Result<BootstrapRunRecords, ApiError> {
    if user_message.trim().is_empty() || user_message.len() > 100_000 {
        return Err(ApiError::Validation(
            "Work must contain between 1 and 100,000 bytes".into(),
        ));
    }
    if request_id.len() > 200 {
        return Err(ApiError::Validation("Idempotency key is too long".into()));
    }
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
    if let Some(row) = sqlx::query(
        "SELECT r.*, q.user_message, q.instructions FROM agent_runs r LEFT JOIN work_queue q ON q.run_id = r.id WHERE request_id = $1",
    )
    .bind(request_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    {
        if row.get::<String, _>("owner_id") != owner {
            return Err(ApiError::NotFound);
        }
        if row.get::<String, _>("bot_id") != bot_id
            || row.get::<Option<String>, _>("source_message_id").as_deref() != Some(source_message_id)
            || row.get::<Option<String>, _>("user_message").as_deref() != Some(user_message.trim())
        {
            return Err(ApiError::Conflict(
                "This request key was already used for different work".into(),
            ));
        }
        return Ok(BootstrapRunRecords {
            run_id: row.get("id"),
            request_id: request_id.into(),
            conversation_id: row.get("conversation_id"),
            assistant_message_id: row.get("assistant_message_id"),
            computer_id: row.get("computer_id"),
            model: row.get("model"),
            instructions: row.get("instructions"),
            is_new_run: false,
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

    let source_row = sqlx::query(
        "SELECT body, sequence, conversation_id FROM messages WHERE id = $1 AND author_kind = 'human'",
    )
    .bind(source_message_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    .ok_or(ApiError::NotFound)?;
    if source_row.get::<String, _>("conversation_id") != conversation_id {
        return Err(ApiError::NotFound);
    }
    if source_row.get::<String, _>("body") != user_message.trim() {
        return Err(ApiError::Validation(
            "source message body does not match work payload".into(),
        ));
    }

    let source_sequence: i64 = source_row.get("sequence");

    crate::groups::assert_bot_may_use_conversation(tx, owner, bot_id, conversation_id).await?;

    let bot = sqlx::query(
        "SELECT b.name, b.model, b.system_prompt, b.engine_preference, b.computer_id FROM bots b JOIN sandboxes s ON s.id = b.computer_id AND s.owner_id = b.owner_id WHERE b.id = $1 AND b.owner_id = $2 AND s.state <> 'archived' FOR SHARE OF b, s",
    )
    .bind(bot_id)
    .bind(owner)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
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

    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(format!("conversation-seq:{conversation_id}"))
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
    let sequence: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sequence), 0) + 1 FROM messages WHERE conversation_id = $1",
    )
    .bind(conversation_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(db_error)?;

    let assistant_message_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO messages (id, conversation_id, role, body, status, sequence, model, author_kind, author_bot_id)
        VALUES ($1, $2, 'assistant', '', 'pending', $3, $4, 'bot', $5)
        "#,
    )
    .bind(&assistant_message_id)
    .bind(conversation_id)
    .bind(sequence)
    .bind(&model)
    .bind(bot_id)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;

    let run_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO agent_runs (id, owner_id, request_id, bot_id, conversation_id, computer_id, model, status, assistant_message_id, source_message_id, group_context_through_sequence) VALUES ($1,$2,$3,$4,$5,$6,$7,'queued',$8,$9,$10)",
    )
    .bind(&run_id)
    .bind(owner)
    .bind(request_id)
    .bind(bot_id)
    .bind(conversation_id)
    .bind(&computer_id)
    .bind(&model)
    .bind(&assistant_message_id)
    .bind(source_message_id)
    .bind(source_sequence)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    sqlx::query(
        "INSERT INTO work_queue (run_id, user_message, instructions, engine_preference) VALUES ($1,$2,$3,$4)",
    )
    .bind(&run_id)
    .bind(user_message.trim())
    .bind(&instructions)
    .bind(&engine)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    sqlx::query("INSERT INTO run_events (request_id, event_type, payload_json) VALUES ($1, 'queued', $2)")
        .bind(request_id)
        .bind(serde_json::json!({"status":"queued", "detail":"Work saved. Waiting for an available computer."}))
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
    sqlx::query("UPDATE conversations SET updated_at = NOW() WHERE id = $1")
        .bind(conversation_id)
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;

    Ok(BootstrapRunRecords {
        run_id,
        request_id: request_id.into(),
        conversation_id: conversation_id.to_string(),
        assistant_message_id,
        computer_id,
        model,
        instructions,
        is_new_run: true,
    })
}

pub async fn enqueue_delegated_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &str,
    request_id: &str,
    bot_id: &str,
    conversation_id: Option<&str>,
    message: &str,
    delegation_id: &str,
) -> Result<BootstrapRunRecords, ApiError> {
    let records = enqueue_in_transaction(tx, owner, request_id, bot_id, conversation_id, message)
        .await?;
    sqlx::query(
        "UPDATE work_queue SET delegation_id = $2, provenance_kind = 'bot_delegation' WHERE run_id = $1",
    )
    .bind(&records.run_id)
    .bind(delegation_id)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    Ok(records)
}

const MAX_DELEGATION_RETURN_RESULT_CHARS: usize = 12_000;

/// Queue exactly one source-Bot continuation after a delegated target reaches a terminal state.
pub async fn enqueue_delegation_return_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    delegation_id: &str,
) -> Result<Option<String>, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT d.*,
               tb.name AS target_bot_name,
               tr.status AS target_run_status,
               tr.id AS target_run_id,
               am.body AS target_result_body
        FROM bot_delegations d
        JOIN bots tb ON tb.id = d.target_bot_id
        LEFT JOIN agent_runs tr ON tr.id = d.target_run_id
        LEFT JOIN messages am ON am.id = tr.assistant_message_id
        WHERE d.id = $1
        FOR UPDATE OF d
        "#,
    )
    .bind(delegation_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    .ok_or(ApiError::NotFound)?;

    let return_policy: String = row.get("return_policy");
    if return_policy != "resume_source" {
        return Ok(None);
    }
    let resume_status: Option<String> = row.get("resume_status");
    let source_resume_run_id: Option<String> = row.get("source_resume_run_id");
    if source_resume_run_id.is_some() || resume_status.is_some() {
        return Ok(source_resume_run_id);
    }

    let owner: String = row.get("owner_id");
    let source_bot_id: String = row.get("source_bot_id");
    let source_conversation_id: String = row.get("source_conversation_id");
    let instruction: String = row.get("instruction");
    let target_bot_name: String = row.get("target_bot_name");
    let target_run_id: Option<String> = row.get("target_run_id");
    let target_run_status: Option<String> = row.get("target_run_status");
    let target_status = target_run_status.unwrap_or_else(|| "failed".into());
    let mut target_result: String = row
        .get::<Option<String>, _>("target_result_body")
        .unwrap_or_default();
    if target_result.len() > MAX_DELEGATION_RETURN_RESULT_CHARS {
        target_result.truncate(MAX_DELEGATION_RETURN_RESULT_CHARS);
        target_result.push('…');
    }

    let conv_type: String = sqlx::query_scalar(
        "SELECT conversation_type FROM conversations WHERE id = $1 AND owner_id = $2",
    )
    .bind(&source_conversation_id)
    .bind(&owner)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    .ok_or(ApiError::NotFound)?;

    if conv_type == "group" {
        let active: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
              SELECT 1 FROM conversation_participants
              WHERE conversation_id = $1 AND bot_id = $2 AND owner_id = $3 AND left_at IS NULL
            )
            "#,
        )
        .bind(&source_conversation_id)
        .bind(&source_bot_id)
        .bind(&owner)
        .fetch_optional(&mut **tx)
        .await
        .map_err(db_error)?
        .unwrap_or(false);
        if !active {
            sqlx::query(
                r#"
                UPDATE bot_delegations
                SET resume_status = 'skipped',
                    resume_error = 'source bot is not an active group participant',
                    resume_created_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(delegation_id)
            .execute(&mut **tx)
            .await
            .map_err(db_error)?;
            return Ok(None);
        }
    }

    let user_message = crate::delegation::format_delegation_return_user_message(
        &target_bot_name,
        &instruction,
        &target_status,
        &target_result,
        target_run_id.as_deref().unwrap_or(""),
    );

    let request_id = format!("delegation-return:{delegation_id}");
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

    crate::groups::assert_bot_may_use_conversation(
        tx,
        &owner,
        &source_bot_id,
        &source_conversation_id,
    )
    .await?;

    let bot = sqlx::query(
        "SELECT b.name, b.model, b.system_prompt, b.engine_preference, b.computer_id FROM bots b JOIN sandboxes s ON s.id = b.computer_id AND s.owner_id = b.owner_id WHERE b.id = $1 AND b.owner_id = $2 AND s.state <> 'archived' FOR SHARE OF b, s",
    )
    .bind(&source_bot_id)
    .bind(&owner)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    .ok_or_else(|| ApiError::Validation("Source bot computer unavailable".into()))?;

    let model: String = bot.get("model");
    let bot_name: String = bot.get("name");
    let system_prompt: String = bot.get("system_prompt");
    let context: Option<String> =
        sqlx::query_scalar("SELECT content FROM bot_context WHERE bot_id = $1")
            .bind(&source_bot_id)
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

    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(format!("conversation-seq:{source_conversation_id}"))
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
    let sequence: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sequence), 0) + 1 FROM messages WHERE conversation_id = $1",
    )
    .bind(&source_conversation_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(db_error)?;

    let system_message_id = Uuid::new_v4().to_string();
    let system_body = format!(
        "Delegated work returned from {target_bot_name} ({target_status})."
    );
    let provenance = serde_json::json!({
        "kind": "delegation_return",
        "delegationId": delegation_id,
        "targetRunId": target_run_id,
        "targetStatus": target_status,
    });
    sqlx::query(
        r#"
        INSERT INTO messages (id, conversation_id, role, body, status, sequence, author_kind, provenance)
        VALUES ($1, $2, 'user', $3, 'complete', $4, 'system', $5)
        "#,
    )
    .bind(&system_message_id)
    .bind(&source_conversation_id)
    .bind(&system_body)
    .bind(sequence)
    .bind(&provenance)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;

    let assistant_message_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO messages (id, conversation_id, role, body, status, sequence, model, author_kind, author_bot_id)
        VALUES ($1, $2, 'assistant', '', 'pending', $3, $4, 'bot', $5)
        "#,
    )
    .bind(&assistant_message_id)
    .bind(&source_conversation_id)
    .bind(sequence + 1)
    .bind(&model)
    .bind(&source_bot_id)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;

    let run_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO agent_runs (
            id, owner_id, request_id, bot_id, conversation_id, computer_id, model,
            status, assistant_message_id, source_message_id
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,'queued',$8,$9)
        "#,
    )
    .bind(&run_id)
    .bind(&owner)
    .bind(&request_id)
    .bind(&source_bot_id)
    .bind(&source_conversation_id)
    .bind(&computer_id)
    .bind(&model)
    .bind(&assistant_message_id)
    .bind(&system_message_id)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;

    sqlx::query(
        r#"
        INSERT INTO work_queue (run_id, user_message, instructions, engine_preference, delegation_id, provenance_kind)
        VALUES ($1,$2,$3,$4,$5,'delegation_return')
        "#,
    )
    .bind(&run_id)
    .bind(&user_message)
    .bind(&instructions)
    .bind(&engine)
    .bind(delegation_id)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;

    sqlx::query(
        r#"
        UPDATE bot_delegations
        SET source_resume_run_id = $2,
            resume_status = 'queued',
            resume_created_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(delegation_id)
    .bind(&run_id)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;

    sqlx::query("INSERT INTO run_events (request_id, event_type, payload_json) VALUES ($1, 'queued', $2)")
        .bind(&request_id)
        .bind(serde_json::json!({"status":"queued","provenanceKind":"delegation_return"}))
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;

    Ok(Some(run_id))
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
    sqlx::query(
        r#"
        UPDATE group_message_recipients
        SET status = 'running', updated_at = NOW()
        WHERE run_id = $1 AND status = 'queued'
        "#,
    )
    .bind(&run_id)
    .execute(&mut *tx)
    .await?;
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
    if let Err(err) = crate::delegation::on_target_run_claimed(pool, &input.records.run_id).await {
        tracing::warn!(run_id = %input.records.run_id, error = %err, "could not mark delegation running");
    }
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
