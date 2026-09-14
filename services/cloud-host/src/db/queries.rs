use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use agent_core::DEFAULT_MODEL;
use sprite_computer::sprite_name_for_sandbox;

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
    let result = sqlx::query(
        r#"
        UPDATE agent_runs
        SET status = 'interrupted',
            error_code = 'host_restart',
            updated_at = NOW(),
            finished_at = COALESCE(finished_at, NOW())
        WHERE status = 'running'
        "#,
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
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
    let row: Option<(String,)> =
        sqlx::query_as("SELECT body FROM messages WHERE id = $1")
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
}

pub async fn bootstrap_run(
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
    let model = model
        .filter(|m| !m.trim().is_empty())
        .unwrap_or(DEFAULT_MODEL)
        .to_string();

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    if let Some(existing) = find_run_by_request_id(pool, request_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
    {
        return Ok(BootstrapRunRecords {
            run_id: existing.id,
            request_id: existing.request_id,
            conversation_id: existing.conversation_id,
            assistant_message_id: existing.assistant_message_id.unwrap_or_default(),
            computer_id: existing.computer_id.unwrap_or_else(|| computer_id.to_string()),
            model: existing.model,
            instructions: instructions.to_string(),
        });
    }

    let now = Utc::now();
    sqlx::query(
        r#"
        INSERT INTO bots (id, name, system_prompt, model, computer_enabled, computer_id, created_at, updated_at)
        VALUES ($1, $2, $3, $4, TRUE, $5, $6, $6)
        ON CONFLICT (id) DO UPDATE SET
            name = EXCLUDED.name,
            system_prompt = EXCLUDED.system_prompt,
            model = EXCLUDED.model,
            computer_id = EXCLUDED.computer_id,
            updated_at = EXCLUDED.updated_at
        "#,
    )
    .bind(bot_id)
    .bind(bot_name)
    .bind(instructions)
    .bind(&model)
    .bind(computer_id)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let sprite_resource = sprite_name_for_sandbox(computer_id);
    let sandbox_id = format!("sandbox-{computer_id}");
    sqlx::query(
        r#"
        INSERT INTO sandboxes (id, provider, provider_resource_id, state, last_used_at, created_at, updated_at)
        VALUES ($1, 'fly_sprite', $2, 'active', $3, $3, $3)
        ON CONFLICT (id) DO UPDATE SET
            provider_resource_id = EXCLUDED.provider_resource_id,
            last_used_at = EXCLUDED.last_used_at,
            updated_at = EXCLUDED.updated_at
        "#,
    )
    .bind(&sandbox_id)
    .bind(&sprite_resource)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let conversation_id = if let Some(id) = conversation_id.filter(|c| !c.is_empty()) {
        id.to_string()
    } else {
        Uuid::new_v4().to_string()
    };

    sqlx::query(
        r#"
        INSERT INTO conversations (id, bot_id, created_at, updated_at)
        VALUES ($1, $2, $3, $3)
        ON CONFLICT (id) DO UPDATE SET updated_at = EXCLUDED.updated_at
        "#,
    )
    .bind(&conversation_id)
    .bind(bot_id)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let user_message_id = Uuid::new_v4().to_string();
    let assistant_message_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO messages (id, conversation_id, role, kind, body, status, sequence, created_at, updated_at)
        VALUES ($1, $2, 'user', 'chat', $3, 'complete', 1, $4, $4)
        "#,
    )
    .bind(&user_message_id)
    .bind(&conversation_id)
    .bind(user_message)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        r#"
        INSERT INTO messages (id, conversation_id, role, kind, body, status, model, sequence, created_at, updated_at)
        VALUES ($1, $2, 'assistant', 'chat', '', 'streaming', $3, 2, $4, $4)
        "#,
    )
    .bind(&assistant_message_id)
    .bind(&conversation_id)
    .bind(&model)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let run_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO agent_runs (
            id, request_id, bot_id, conversation_id, computer_id, model, status,
            assistant_message_id, step_count, started_at, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, 'running', $7, 0, $8, $8, $8)
        "#,
    )
    .bind(&run_id)
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
    })
}
