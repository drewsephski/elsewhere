use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use agent_core::{compose_runtime_instruction_snapshot, RuntimeIdentityInput};

use agent_core::DEFAULT_MODEL;
use sprite_computer::sprite_name_for_sandbox;

use crate::auth::LEGACY_LOCAL_OWNER;
use crate::error::ApiError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AgentRunRow {
    pub id: String,
    pub request_id: String,
    pub bot_id: String,
    pub conversation_id: String,
    pub computer_id: Option<String>,
    pub model: String,
    pub status: String,
    pub error_code: Option<String>,
    pub step_count: i64,
    pub assistant_message_id: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RunEventRow {
    pub id: i64,
    pub request_id: String,
    pub event_type: String,
    pub payload_json: Value,
    pub created_at: DateTime<Utc>,
}

pub async fn mark_interrupted_runs(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let restarted: Vec<(String, Option<String>)> = sqlx::query_as(
        r#"
        UPDATE agent_runs
        SET status = 'interrupted',
            error_code = 'host_restart',
            updated_at = NOW(),
            finished_at = COALESCE(finished_at, NOW())
        WHERE status = 'running'
        RETURNING request_id, assistant_message_id
        "#,
    )
    .fetch_all(&mut *tx)
    .await?;

    for (_, assistant_message_id) in &restarted {
        if let Some(message_id) = assistant_message_id {
            sqlx::query(
                r#"
                UPDATE messages
                SET status = 'interrupted', updated_at = NOW()
                WHERE id = $1 AND status IN ('pending', 'streaming')
                "#,
            )
            .bind(message_id)
            .execute(&mut *tx)
            .await?;
        }
    }

    for (request_id, _) in &restarted {
        let payload = json!({
            "status": "interrupted",
            "code": "host_restart",
            "detail": "cloud host restarted while run was active",
        });
        sqlx::query(
            "INSERT INTO run_events (request_id, event_type, payload_json) VALUES ($1, 'host_restart', $2)",
        )
        .bind(request_id)
        .bind(&payload)
        .execute(&mut *tx)
        .await?;
    }

    for (request_id, _) in &restarted {
        if let Some(run_id) = sqlx::query_scalar::<_, String>(
            "SELECT id FROM agent_runs WHERE request_id = $1",
        )
        .bind(request_id)
        .fetch_optional(&mut *tx)
        .await?
        {
            if let Err(err) = crate::run_lifecycle::synchronize_run_terminal_in_tx(
                &mut tx,
                &run_id,
                "interrupted",
                Some("host_restart"),
            )
            .await
            {
                tracing::warn!(
                    request_id = %request_id,
                    run_id = %run_id,
                    error = %err,
                    repair_action = "restart_interrupted_sync",
                    "could not synchronize interrupted run lifecycle"
                );
            }
        }
    }

    sqlx::query("UPDATE agent_runs SET execution_released_at = NOW(), results_note = CASE WHEN status = 'completed' THEN 'The runner restarted while saving results. The summary is available; some files may remain on the computer.' ELSE results_note END WHERE started_at IS NOT NULL AND execution_released_at IS NULL")
        .execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(restarted.len() as u64)
}

pub async fn find_run_by_id(pool: &PgPool, run_id: &str) -> Result<Option<AgentRunRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT id, request_id, bot_id, conversation_id, computer_id, model, status,
               error_code, step_count, assistant_message_id, started_at, finished_at
        FROM agent_runs WHERE id = $1
        "#,
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
}

pub async fn find_run_for_owner(
    pool: &PgPool,
    owner_id: &str,
    run_id: &str,
) -> Result<Option<AgentRunRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT id, request_id, bot_id, conversation_id, computer_id, model, status,
               error_code, step_count, assistant_message_id, started_at, finished_at
        FROM agent_runs WHERE id = $1 AND owner_id = $2
        "#,
    )
    .bind(run_id)
    .bind(owner_id)
    .fetch_optional(pool)
    .await
}

pub async fn find_run_by_request_id(
    pool: &PgPool,
    request_id: &str,
) -> Result<Option<AgentRunRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT id, request_id, bot_id, conversation_id, computer_id, model, status,
               error_code, step_count, assistant_message_id, started_at, finished_at
        FROM agent_runs WHERE request_id = $1
        "#,
    )
    .bind(request_id)
    .fetch_optional(pool)
    .await
}

async fn find_run_by_request_id_tx(
    tx: &mut Transaction<'_, Postgres>,
    request_id: &str,
) -> Result<Option<AgentRunRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT id, request_id, bot_id, conversation_id, computer_id, model, status,
               error_code, step_count, assistant_message_id, started_at, finished_at
        FROM agent_runs WHERE request_id = $1
        "#,
    )
    .bind(request_id)
    .fetch_optional(&mut **tx)
    .await
}

pub async fn list_run_events_after(
    pool: &PgPool,
    request_id: &str,
    after_id: i64,
    limit: i64,
) -> Result<Vec<RunEventRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT id, request_id, event_type, payload_json, created_at
        FROM run_events
        WHERE request_id = $1 AND id > $2
        ORDER BY id ASC
        LIMIT $3
        "#,
    )
    .bind(request_id)
    .bind(after_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn assistant_message_body(
    pool: &PgPool,
    message_id: &str,
) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as("SELECT body FROM messages WHERE id = $1")
        .bind(message_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.0))
}

#[derive(Debug, Clone)]
pub struct BootstrapRunRecords {
    pub run_id: String,
    pub request_id: String,
    pub conversation_id: String,
    pub assistant_message_id: String,
    pub computer_id: String,
    pub model: String,
    pub instructions: String,
    pub is_new_run: bool,
}

async fn advisory_lock_request(tx: &mut Transaction<'_, Postgres>, request_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(request_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn advisory_lock_conversation_sequence(
    tx: &mut Transaction<'_, Postgres>,
    conversation_id: &str,
) -> Result<(), sqlx::Error> {
    let key = format!("conversation-seq:{conversation_id}");
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(&key)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn next_message_sequence_tx(
    tx: &mut Transaction<'_, Postgres>,
    conversation_id: &str,
) -> Result<i64, sqlx::Error> {
    advisory_lock_conversation_sequence(tx, conversation_id).await?;
    let row: (Option<i64>,) = sqlx::query_as(
        "SELECT MAX(sequence) FROM messages WHERE conversation_id = $1",
    )
    .bind(conversation_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(row.0.unwrap_or(0) + 1)
}

/// Dev E2E computer id: maps to `ELSEWHERE_TEST_SPRITE` when set (stable Fly Sprite reuse).
pub const E2E_COMPUTER_ID: &str = "elsewhere-cloud-e2e";

fn resolve_sprite_resource(computer_id: &str) -> String {
    if computer_id == E2E_COMPUTER_ID {
        if let Ok(name) = std::env::var("ELSEWHERE_TEST_SPRITE") {
            let name = name.trim();
            if !name.is_empty() {
                return name.to_string();
            }
        }
    }
    sprite_name_for_sandbox(computer_id)
}

pub async fn bootstrap_run(
    pool: &PgPool,
    owner_id: &str,
    request_id: &str,
    bot_id: &str,
    bot_name: &str,
    instructions: &str,
    model: Option<&str>,
    computer_id: &str,
    conversation_id: Option<&str>,
    user_message: &str,
    engine_preference: &str,
) -> Result<BootstrapRunRecords, ApiError> {
    let model = model
        .filter(|m| !m.trim().is_empty())
        .unwrap_or(DEFAULT_MODEL)
        .to_string();

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    advisory_lock_request(&mut tx, request_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    if let Some(existing) = find_run_by_request_id_tx(&mut tx, request_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
    {
        tx.commit()
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        return Ok(BootstrapRunRecords {
            run_id: existing.id,
            request_id: existing.request_id,
            conversation_id: existing.conversation_id,
            assistant_message_id: existing.assistant_message_id.unwrap_or_default(),
            computer_id: existing.computer_id.unwrap_or_else(|| computer_id.to_string()),
            model: existing.model,
            instructions: instructions.to_string(),
            is_new_run: false,
        });
    }

    let now = Utc::now();
    sqlx::query(
        r#"
        INSERT INTO bots (id, owner_id, name, system_prompt, model, computer_enabled, computer_id, engine_preference, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, TRUE, $6, $7, $8, $8)
        ON CONFLICT (id) DO UPDATE SET
            name = EXCLUDED.name,
            system_prompt = EXCLUDED.system_prompt,
            model = EXCLUDED.model,
            computer_id = EXCLUDED.computer_id,
            engine_preference = EXCLUDED.engine_preference,
            updated_at = EXCLUDED.updated_at
        "#,
    )
    .bind(bot_id)
    .bind(owner_id)
    .bind(bot_name)
    .bind(instructions)
    .bind(&model)
    .bind(computer_id)
    .bind(engine_preference)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let sprite_resource = resolve_sprite_resource(computer_id);
    let sandbox_id = computer_id.to_string();
    if let Some((legacy_id,)) = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM sandboxes WHERE provider = 'fly_sprite' AND provider_resource_id = $1",
    )
    .bind(&sprite_resource)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    {
        if legacy_id != sandbox_id {
            sqlx::query("DELETE FROM sandboxes WHERE id = $1")
                .bind(&legacy_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
        }
    }
    sqlx::query(
        r#"
        INSERT INTO sandboxes (id, owner_id, provider, provider_resource_id, state, last_used_at, created_at, updated_at)
        VALUES ($1, $2, 'fly_sprite', $3, 'active', $4, $4, $4)
        ON CONFLICT (id) DO UPDATE SET
            provider_resource_id = EXCLUDED.provider_resource_id,
            last_used_at = EXCLUDED.last_used_at,
            updated_at = EXCLUDED.updated_at
        "#,
    )
    .bind(&sandbox_id)
    .bind(owner_id)
    .bind(&sprite_resource)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let conversation_id = if let Some(id) = conversation_id.filter(|c| !c.is_empty()) {
        let row: Option<(String, String)> =
            sqlx::query_as("SELECT bot_id, owner_id FROM conversations WHERE id = $1")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
        match row {
            Some((_existing_bot, existing_owner)) if existing_owner != owner_id => {
                return Err(ApiError::NotFound);
            }
            Some((existing_bot, _)) if existing_bot != bot_id => {
                return Err(ApiError::Conflict(
                    "conversation belongs to a different bot".into(),
                ));
            }
            Some(_) => id.to_string(),
            None => {
                return Err(ApiError::Validation(
                    "conversationId does not exist".into(),
                ));
            }
        }
    } else {
        crate::conversation::get_or_create_primary_conversation_id_in_tx(&mut tx, owner_id, bot_id)
            .await?
    };

    sqlx::query("UPDATE conversations SET updated_at = $1 WHERE id = $2")
        .bind(now)
        .bind(&conversation_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let user_sequence = next_message_sequence_tx(&mut tx, &conversation_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let assistant_sequence = user_sequence + 1;

    let user_message_id = Uuid::new_v4().to_string();
    let assistant_message_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO messages (id, conversation_id, role, kind, body, status, sequence, created_at, updated_at)
        VALUES ($1, $2, 'user', 'chat', $3, 'complete', $4, $5, $5)
        "#,
    )
    .bind(&user_message_id)
    .bind(&conversation_id)
    .bind(user_message)
    .bind(user_sequence)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        r#"
        INSERT INTO messages (id, conversation_id, role, kind, body, status, model, sequence, created_at, updated_at)
        VALUES ($1, $2, 'assistant', 'chat', '', 'streaming', $3, $4, $5, $5)
        "#,
    )
    .bind(&assistant_message_id)
    .bind(&conversation_id)
    .bind(&model)
    .bind(assistant_sequence)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let run_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO agent_runs (
            id, owner_id, request_id, bot_id, conversation_id, computer_id, model, status,
            assistant_message_id, step_count, started_at, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, 'running', $8, 0, $9, $9, $9)
        "#,
    )
    .bind(&run_id)
    .bind(owner_id)
    .bind(request_id)
    .bind(bot_id)
    .bind(&conversation_id)
    .bind(computer_id)
    .bind(&model)
    .bind(&assistant_message_id)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(BootstrapRunRecords {
        run_id,
        request_id: request_id.to_string(),
        conversation_id,
        assistant_message_id,
        computer_id: computer_id.to_string(),
        model,
        instructions: instructions.to_string(),
        is_new_run: true,
    })
}

/// Product run bootstrap: load persisted bot + computer; never trust client instructions.
pub async fn bootstrap_run_from_bot(
    pool: &PgPool,
    owner_id: &str,
    request_id: &str,
    bot_id: &str,
    conversation_id: Option<&str>,
    user_message: &str,
) -> Result<BootstrapRunRecords, ApiError> {
    let bot = crate::db::resources::get_bot_for_owner(pool, owner_id, bot_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::NotFound)?;
    let computer_id = bot
        .computer_id
        .as_deref()
        .filter(|c| !c.is_empty())
        .ok_or_else(|| ApiError::Validation("bot has no assigned computer".into()))?;

    crate::db::resources::ensure_computer_provisioned(pool, owner_id, computer_id).await?;

    let saved_context: Option<String> =
        sqlx::query_scalar("SELECT content FROM bot_context WHERE bot_id = $1")
            .bind(&bot.id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    let instructions = compose_runtime_instruction_snapshot(&RuntimeIdentityInput {
        bot_name: bot.name.clone(),
        role_instructions: bot.system_prompt.clone(),
        saved_context: saved_context.filter(|value| !value.is_empty()),
    });

    bootstrap_run(
        pool,
        owner_id,
        request_id,
        &bot.id,
        &bot.name,
        &instructions,
        Some(bot.model.as_str()),
        computer_id,
        conversation_id,
        user_message,
        &bot.engine_preference,
    )
    .await
}

pub async fn bootstrap_run_legacy(
    pool: &PgPool,
    request_id: &str,
    bot_id: &str,
    bot_name: &str,
    instructions: &str,
    model: Option<&str>,
    computer_id: &str,
    conversation_id: Option<&str>,
    user_message: &str,
) -> Result<BootstrapRunRecords, ApiError> {
    bootstrap_run(
        pool,
        LEGACY_LOCAL_OWNER,
        request_id,
        bot_id,
        bot_name,
        instructions,
        model,
        computer_id,
        conversation_id,
        user_message,
        "responses",
    )
    .await
}
