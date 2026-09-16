//! Internal work-admission kernel: shared invariants and typed admission intents.

use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::conversation::get_or_create_primary_conversation_id_in_tx;
use crate::db::queries::BootstrapRunRecords;
use crate::error::ApiError;
use crate::groups;
use crate::memory::retrieval::{retrieve_scored_in_tx, RetrievedMemory, MAX_RUN_MEMORY_ITEMS};
use crate::memory::snapshot::{compose_with_memories, persist_run_memories};
use crate::skills::{
    explicit_invocation_idempotency_mismatch, explicit_invocation_on_run,
    persist_run_skills_for_admission, SkillAdmissionInput,
};

fn db_error(error: sqlx::Error) -> ApiError {
    ApiError::Internal(error.to_string())
}

pub(crate) struct BotExecutionSnapshot {
    pub model: String,
    pub computer_id: String,
    pub engine: String,
    pub instructions: String,
    pub bot_name: String,
    pub memories: Vec<RetrievedMemory>,
}

pub(crate) fn validate_admission_message(
    message: &str,
    attachment_count: usize,
) -> Result<(), ApiError> {
    if message.len() > 100_000 {
        return Err(ApiError::Validation(
            "Work must contain between 1 and 100,000 bytes".into(),
        ));
    }
    if message.trim().is_empty() && attachment_count == 0 {
        return Err(ApiError::Validation(
            "Work must contain between 1 and 100,000 bytes".into(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_request_id(request_id: &str) -> Result<(), ApiError> {
    if request_id.len() > 200 {
        return Err(ApiError::Validation("Idempotency key is too long".into()));
    }
    Ok(())
}

pub(crate) async fn acquire_owner_request_locks(
    tx: &mut Transaction<'_, Postgres>,
    owner: &str,
    request_id: &str,
) -> Result<(), ApiError> {
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
    Ok(())
}

pub(crate) async fn enforce_active_run_limit(
    tx: &mut Transaction<'_, Postgres>,
    owner: &str,
) -> Result<(), ApiError> {
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
    Ok(())
}

pub(crate) async fn load_bot_execution_snapshot(
    tx: &mut Transaction<'_, Postgres>,
    owner: &str,
    bot_id: &str,
    retrieval_query: &str,
) -> Result<BotExecutionSnapshot, ApiError> {
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
    let memories = retrieve_scored_in_tx(
        tx,
        owner,
        bot_id,
        retrieval_query,
        MAX_RUN_MEMORY_ITEMS as i64,
    )
    .await?;
    let instructions = compose_with_memories(
        bot_name.clone(),
        system_prompt,
        context.filter(|value| !value.is_empty()),
        &memories,
    );

    Ok(BotExecutionSnapshot {
        model,
        computer_id: bot.get("computer_id"),
        engine: bot.get("engine_preference"),
        instructions,
        bot_name,
        memories,
    })
}

pub(crate) enum WorkAdmissionIntent<'a> {
    HumanMessage {
        bot_id: &'a str,
        conversation_id: Option<&'a str>,
        message: &'a str,
        skills: &'a SkillAdmissionInput,
        attachment_ids: &'a [String],
    },
    ChannelMessage {
        bot_id: &'a str,
        conversation_id: &'a str,
        message: &'a str,
        skills: &'a SkillAdmissionInput,
        origin_provider: &'a str,
    },
}

pub(crate) async fn admit(
    tx: &mut Transaction<'_, Postgres>,
    owner: &str,
    request_id: &str,
    intent: WorkAdmissionIntent<'_>,
) -> Result<BootstrapRunRecords, ApiError> {
    match intent {
        WorkAdmissionIntent::HumanMessage {
            bot_id,
            conversation_id,
            message,
            skills,
            attachment_ids,
        } => {
            admit_human_message(
                tx,
                owner,
                request_id,
                bot_id,
                conversation_id,
                message,
                skills,
                "web",
                None,
                attachment_ids,
            )
            .await
        }
        WorkAdmissionIntent::ChannelMessage {
            bot_id,
            conversation_id,
            message,
            skills,
            origin_provider,
        } => {
            admit_human_message(
                tx,
                owner,
                request_id,
                bot_id,
                Some(conversation_id),
                message,
                skills,
                "channel",
                Some(origin_provider),
                &[],
            )
            .await
        }
    }
}

async fn admit_human_message(
    tx: &mut Transaction<'_, Postgres>,
    owner: &str,
    request_id: &str,
    bot_id: &str,
    conversation_id: Option<&str>,
    message: &str,
    skills: &SkillAdmissionInput,
    origin_kind: &str,
    origin_provider: Option<&str>,
    attachment_ids: &[String],
) -> Result<BootstrapRunRecords, ApiError> {
    validate_admission_message(message, attachment_ids.len())?;
    validate_request_id(request_id)?;
    acquire_owner_request_locks(tx, owner, request_id).await?;

    let attachment_fp = crate::attachments::store::fingerprint(attachment_ids);
    if let Some(row) = sqlx::query(
        "SELECT r.*, q.instructions, q.user_message FROM agent_runs r LEFT JOIN work_queue q ON q.run_id = r.id WHERE request_id = $1",
    )
    .bind(request_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    {
        if row.get::<String, _>("owner_id") != owner {
            return Err(ApiError::NotFound);
        }
        let stored_fp = crate::attachments::store::attachment_ids_for_run_in_tx(
            tx,
            &row.get::<String, _>("id"),
        )
        .await?;
        if row.get::<String, _>("bot_id") != bot_id
            || row.get::<Option<String>, _>("user_message").as_deref() != Some(message.trim())
            || conversation_id.is_some_and(|id| id != row.get::<String, _>("conversation_id"))
            || crate::attachments::store::fingerprint(&stored_fp) != attachment_fp
        {
            return Err(ApiError::Conflict(
                "This request key was already used for different work".into(),
            ));
        }
        let run_id: String = row.get("id");
        let stored_explicit = explicit_invocation_on_run(tx, &run_id).await?;
        if explicit_invocation_idempotency_mismatch(skills, stored_explicit) {
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

    enforce_active_run_limit(tx, owner).await?;
    let snapshot = load_bot_execution_snapshot(tx, owner, bot_id, message).await?;

    let conversation_id = if let Some(id) = conversation_id {
        groups::assert_bot_may_use_conversation(tx, owner, bot_id, id).await?;
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
    .bind(&snapshot.model)
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
    sqlx::query(
        "INSERT INTO agent_runs (id, owner_id, request_id, bot_id, conversation_id, computer_id, model, status, assistant_message_id, origin_kind, origin_provider) VALUES ($1,$2,$3,$4,$5,$6,$7,'queued',$8,$9,$10)",
    )
    .bind(&run_id)
    .bind(owner)
    .bind(request_id)
    .bind(bot_id)
    .bind(&conversation_id)
    .bind(&snapshot.computer_id)
    .bind(&snapshot.model)
    .bind(&assistant_message_id)
    .bind(origin_kind)
    .bind(origin_provider)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    sqlx::query(
        "INSERT INTO work_queue (run_id, user_message, instructions, engine_preference, provenance_kind) VALUES ($1,$2,$3,$4,$5)",
    )
    .bind(&run_id)
    .bind(message.trim())
    .bind(&snapshot.instructions)
    .bind(&snapshot.engine)
    .bind(if origin_kind == "channel" {
        Some(origin_kind)
    } else {
        None
    })
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    sqlx::query(
        "INSERT INTO run_events (request_id, event_type, payload_json) VALUES ($1, 'queued', $2)",
    )
    .bind(request_id)
    .bind(serde_json::json!({
        "status": "queued",
        "detail": "Work saved. Waiting for an available computer."
    }))
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    sqlx::query("UPDATE conversations SET updated_at = NOW() WHERE id = $1")
        .bind(&conversation_id)
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
    persist_run_skills_for_admission(tx, owner, bot_id, &run_id, skills).await?;
    persist_run_memories(tx, owner, bot_id, &run_id, &snapshot.memories).await?;

    if !attachment_ids.is_empty() {
        let staged = crate::attachments::store::load_staged_for_admission(
            tx,
            owner,
            Some(bot_id),
            Some(&conversation_id),
            attachment_ids,
        )
        .await?;
        crate::attachments::store::attach_to_message_and_run(
            tx,
            &user_message_id,
            &run_id,
            &staged,
        )
        .await?;
    }

    Ok(BootstrapRunRecords {
        run_id,
        request_id: request_id.into(),
        conversation_id,
        assistant_message_id,
        computer_id: snapshot.computer_id,
        model: snapshot.model,
        instructions: snapshot.instructions,
        is_new_run: true,
    })
}
