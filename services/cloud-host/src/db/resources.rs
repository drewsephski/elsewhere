use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use agent_core::canonical_bot_model;

use crate::auth::LEGACY_LOCAL_OWNER;
use crate::error::ApiError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BotRow {
    pub id: String,
    pub owner_id: String,
    pub name: String,
    pub system_prompt: String,
    pub model: String,
    pub computer_id: Option<String>,
    pub engine_preference: String,
    pub avatar_id: String,
    pub learn_from_conversations: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SandboxRow {
    pub id: String,
    pub owner_id: String,
    pub display_name: String,
    pub provider: String,
    pub provider_resource_id: String,
    pub state: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ConversationRow {
    pub id: String,
    pub owner_id: String,
    pub bot_id: Option<String>,
    pub conversation_type: String,
    pub name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn list_bots(pool: &PgPool, owner_id: &str) -> Result<Vec<BotRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT id, owner_id, name, system_prompt, model, computer_id, engine_preference,
               avatar_id, learn_from_conversations, created_at, updated_at
        FROM bots WHERE owner_id = $1 ORDER BY updated_at DESC
        "#,
    )
    .bind(owner_id)
    .fetch_all(pool)
    .await
}

pub async fn get_bot_for_owner(
    pool: &PgPool,
    owner_id: &str,
    bot_id: &str,
) -> Result<Option<BotRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT id, owner_id, name, system_prompt, model, computer_id, engine_preference,
               avatar_id, learn_from_conversations, created_at, updated_at
        FROM bots WHERE id = $1 AND owner_id = $2
        "#,
    )
    .bind(bot_id)
    .bind(owner_id)
    .fetch_optional(pool)
    .await
}

pub async fn insert_bot<'e, E>(
    executor: E,
    owner_id: &str,
    name: &str,
    instructions: &str,
    model: &str,
    computer_id: Option<&str>,
    engine_preference: &str,
    avatar_id: &str,
) -> Result<BotRow, ApiError>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let id = Uuid::new_v4().to_string();
    let now = Utc::now();
    sqlx::query_as(
        r#"
        INSERT INTO bots (
            id, owner_id, name, system_prompt, model, computer_enabled, computer_id,
            engine_preference, avatar_id, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, TRUE, $6, $7, $8, $9, $9)
        RETURNING id, owner_id, name, system_prompt, model, computer_id, engine_preference,
                  avatar_id, learn_from_conversations, created_at, updated_at
        "#,
    )
    .bind(&id)
    .bind(owner_id)
    .bind(name)
    .bind(instructions)
    .bind(model)
    .bind(computer_id)
    .bind(engine_preference)
    .bind(avatar_id)
    .bind(now)
    .fetch_one(executor)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn patch_bot(
    pool: &PgPool,
    owner_id: &str,
    bot_id: &str,
    name: Option<&str>,
    instructions: Option<&str>,
    model: Option<&str>,
    computer_id: Option<Option<&str>>,
    engine_preference: Option<&str>,
    avatar_id: Option<&str>,
    learn_from_conversations: Option<bool>,
) -> Result<Option<BotRow>, ApiError> {
    sqlx::query_as(
        r#"
        UPDATE bots SET
            name = COALESCE($3, name),
            system_prompt = COALESCE($4, system_prompt),
            model = COALESCE($5, model),
            computer_id = CASE WHEN $6 THEN $7 ELSE computer_id END,
            engine_preference = COALESCE($8, engine_preference),
            avatar_id = COALESCE($9, avatar_id),
            learn_from_conversations = COALESCE($10, learn_from_conversations),
            updated_at = NOW()
        WHERE id = $1 AND owner_id = $2
        RETURNING id, owner_id, name, system_prompt, model, computer_id, engine_preference,
                  avatar_id, learn_from_conversations, created_at, updated_at
        "#,
    )
    .bind(bot_id)
    .bind(owner_id)
    .bind(name)
    .bind(instructions)
    .bind(model)
    .bind(computer_id.is_some())
    .bind(computer_id.flatten())
    .bind(engine_preference)
    .bind(avatar_id)
    .bind(learn_from_conversations)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn delete_bot(pool: &PgPool, owner_id: &str, bot_id: &str) -> Result<bool, ApiError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let exists: Option<(String,)> =
        sqlx::query_as("SELECT id FROM bots WHERE id = $1 AND owner_id = $2 FOR UPDATE")
            .bind(bot_id)
            .bind(owner_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    if exists.is_none() {
        return Ok(false);
    }

    let has_active_work: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM agent_runs
            WHERE bot_id = $1
              AND owner_id = $2
              AND status IN ('queued', 'running')
        )
        "#,
    )
    .bind(bot_id)
    .bind(owner_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    if has_active_work {
        return Err(ApiError::Conflict(
            "Stop this Bot's active work before deleting it.".into(),
        ));
    }

    let would_orphan_group: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM conversation_participants p
            JOIN conversations c ON c.id = p.conversation_id
            WHERE p.bot_id = $1
              AND p.owner_id = $2
              AND p.left_at IS NULL
              AND c.conversation_type = 'group'
              AND (
                  SELECT COUNT(*)
                  FROM conversation_participants m
                  WHERE m.conversation_id = c.id
                    AND m.left_at IS NULL
              ) <= 2
        )
        "#,
    )
    .bind(bot_id)
    .bind(owner_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    if would_orphan_group {
        return Err(ApiError::Conflict(
            "Remove this Bot from its two-member group (or delete the group) before deleting it."
                .into(),
        ));
    }

    // Remove dependent rows in FK-safe order. Direct conversations are deleted; group
    // conversations stay, but this bot is detached from them first.
    sqlx::query(
        r#"
        DELETE FROM work_queue
        WHERE routine_id IN (
            SELECT id FROM routines WHERE bot_id = $1 AND owner_id = $2
        )
        "#,
    )
    .bind(bot_id)
    .bind(owner_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query("DELETE FROM routines WHERE bot_id = $1 AND owner_id = $2")
        .bind(bot_id)
        .bind(owner_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        r#"
        DELETE FROM tool_approval_requests
        WHERE run_id IN (
            SELECT id FROM agent_runs WHERE bot_id = $1 AND owner_id = $2
        )
        "#,
    )
    .bind(bot_id)
    .bind(owner_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        r#"
        DELETE FROM work_queue
        WHERE run_id IN (
            SELECT id FROM agent_runs WHERE bot_id = $1 AND owner_id = $2
        )
        "#,
    )
    .bind(bot_id)
    .bind(owner_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        r#"
        DELETE FROM work_results
        WHERE run_id IN (
            SELECT id FROM agent_runs WHERE bot_id = $1 AND owner_id = $2
        )
        "#,
    )
    .bind(bot_id)
    .bind(owner_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        r#"
        DELETE FROM run_events
        WHERE request_id IN (
            SELECT request_id FROM agent_runs WHERE bot_id = $1 AND owner_id = $2
        )
        "#,
    )
    .bind(bot_id)
    .bind(owner_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        r#"
        UPDATE channel_events
        SET run_id = NULL
        WHERE run_id IN (
            SELECT id FROM agent_runs WHERE bot_id = $1 AND owner_id = $2
        )
        "#,
    )
    .bind(bot_id)
    .bind(owner_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        r#"
        UPDATE channel_deliveries
        SET source_run_id = NULL
        WHERE source_run_id IN (
            SELECT id FROM agent_runs WHERE bot_id = $1 AND owner_id = $2
        )
        "#,
    )
    .bind(bot_id)
    .bind(owner_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        r#"
        UPDATE bot_delegations
        SET parent_delegation_id = NULL
        WHERE parent_delegation_id IN (
            SELECT id FROM (
                SELECT id FROM bot_delegations
                WHERE owner_id = $2 AND (source_bot_id = $1 OR target_bot_id = $1)
            ) dangling_parents
        )
        "#,
    )
    .bind(bot_id)
    .bind(owner_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        r#"
        DELETE FROM bot_delegations
        WHERE owner_id = $2 AND (source_bot_id = $1 OR target_bot_id = $1)
        "#,
    )
    .bind(bot_id)
    .bind(owner_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        r#"
        DELETE FROM bot_creations
        WHERE owner_id = $2 AND (source_bot_id = $1 OR created_bot_id = $1)
        "#,
    )
    .bind(bot_id)
    .bind(owner_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query("DELETE FROM run_subagents WHERE bot_id = $1 AND owner_id = $2")
        .bind(bot_id)
        .bind(owner_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query("DELETE FROM agent_runs WHERE bot_id = $1 AND owner_id = $2")
        .bind(bot_id)
        .bind(owner_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        r#"
        DELETE FROM channel_threads
        WHERE owner_id = $2 AND (
            bot_id = $1
            OR conversation_id IN (
                SELECT id FROM conversations WHERE bot_id = $1 AND owner_id = $2
            )
        )
        "#,
    )
    .bind(bot_id)
    .bind(owner_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        r#"
        DELETE FROM conversation_participants
        WHERE bot_id = $1 AND owner_id = $2
        "#,
    )
    .bind(bot_id)
    .bind(owner_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query("DELETE FROM conversation_bot_threads WHERE bot_id = $1")
        .bind(bot_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query("DELETE FROM group_message_recipients WHERE bot_id = $1")
        .bind(bot_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query("UPDATE messages SET author_bot_id = NULL WHERE author_bot_id = $1")
        .bind(bot_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        r#"
        DELETE FROM messages
        WHERE conversation_id IN (
            SELECT id FROM conversations WHERE bot_id = $1 AND owner_id = $2
        )
        "#,
    )
    .bind(bot_id)
    .bind(owner_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query("DELETE FROM conversations WHERE bot_id = $1 AND owner_id = $2")
        .bind(bot_id)
        .bind(owner_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        r#"
        UPDATE channel_connections
        SET default_bot_id = NULL
        WHERE default_bot_id = $1 AND owner_id = $2
        "#,
    )
    .bind(bot_id)
    .bind(owner_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query("DELETE FROM channel_oauth_states WHERE bot_id = $1 AND owner_id = $2")
        .bind(bot_id)
        .bind(owner_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let result = sqlx::query("DELETE FROM bots WHERE id = $1 AND owner_id = $2")
        .bind(bot_id)
        .bind(owner_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(result.rows_affected() > 0)
}

pub async fn list_computers(pool: &PgPool, owner_id: &str) -> Result<Vec<SandboxRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT id, owner_id, display_name, provider, provider_resource_id, state,
               created_at, updated_at, last_used_at
        FROM sandboxes
        WHERE owner_id = $1 AND state <> 'archived'
        ORDER BY updated_at DESC
        "#,
    )
    .bind(owner_id)
    .fetch_all(pool)
    .await
}

pub async fn get_computer_for_owner(
    pool: &PgPool,
    owner_id: &str,
    computer_id: &str,
) -> Result<Option<SandboxRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT id, owner_id, display_name, provider, provider_resource_id, state,
               created_at, updated_at, last_used_at
        FROM sandboxes WHERE id = $1 AND owner_id = $2
        "#,
    )
    .bind(computer_id)
    .bind(owner_id)
    .fetch_optional(pool)
    .await
}

pub async fn insert_computer_placeholder(
    pool: &PgPool,
    owner_id: &str,
    display_name: &str,
) -> Result<SandboxRow, ApiError> {
    let id = Uuid::new_v4().to_string();
    let provider_resource_id = sprite_computer::sprite_name_for_sandbox(&id);
    let now = Utc::now();
    sqlx::query_as(
        r#"
        INSERT INTO sandboxes (
            id, owner_id, display_name, provider, provider_resource_id, state,
            created_at, updated_at
        ) VALUES ($1, $2, $3, 'fly_sprite', $5, 'pending', $4, $4)
        RETURNING id, owner_id, display_name, provider, provider_resource_id, state,
                  created_at, updated_at, last_used_at
        "#,
    )
    .bind(&id)
    .bind(owner_id)
    .bind(display_name)
    .bind(now)
    .bind(&provider_resource_id)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn ensure_computer_provisioned(
    pool: &PgPool,
    owner_id: &str,
    computer_id: &str,
) -> Result<SandboxRow, ApiError> {
    let row = get_computer_for_owner(pool, owner_id, computer_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::NotFound)?;
    if !row.provider_resource_id.is_empty() {
        return Ok(row);
    }
    let resource = sprite_computer::sprite_name_for_sandbox(&row.id);
    let now = Utc::now();
    sqlx::query_as(
        r#"
        UPDATE sandboxes SET
            provider_resource_id = $3,
            state = 'active',
            last_used_at = $4,
            updated_at = $4
        WHERE id = $1 AND owner_id = $2
        RETURNING id, owner_id, display_name, provider, provider_resource_id, state,
                  created_at, updated_at, last_used_at
        "#,
    )
    .bind(computer_id)
    .bind(owner_id)
    .bind(&resource)
    .bind(now)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn archive_computer(
    pool: &PgPool,
    owner_id: &str,
    computer_id: &str,
) -> Result<bool, ApiError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let existing: Option<(String,)> = sqlx::query_as("SELECT id FROM sandboxes WHERE id = $1 AND owner_id = $2 AND state <> 'archived' FOR UPDATE")
        .bind(computer_id).bind(owner_id).fetch_optional(&mut *tx).await.map_err(|e| ApiError::Internal(e.to_string()))?;
    if existing.is_none() {
        return Ok(false);
    }
    let busy: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM agent_runs WHERE computer_id = $1 AND status IN ('queued','running'))")
        .bind(computer_id).fetch_one(&mut *tx).await.map_err(|e| ApiError::Internal(e.to_string()))?;
    if busy {
        return Err(ApiError::Conflict(
            "Cancel or finish this computer's work before archiving it".into(),
        ));
    }
    sqlx::query("UPDATE sandboxes SET state = 'archived', updated_at = NOW() WHERE id = $1")
        .bind(computer_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(true)
}

pub async fn list_runs_for_owner(
    pool: &PgPool,
    owner_id: &str,
    limit: i64,
) -> Result<Vec<crate::db::queries::AgentRunRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT id, request_id, bot_id, conversation_id, computer_id, model, status,
               error_code, step_count, assistant_message_id, started_at, finished_at,
               origin_kind, origin_provider
        FROM agent_runs
        WHERE owner_id = $1
        ORDER BY created_at DESC
        LIMIT $2
        "#,
    )
    .bind(owner_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn list_conversations_for_owner(
    pool: &PgPool,
    owner_id: &str,
    bot_id: Option<&str>,
    limit: i64,
) -> Result<Vec<ConversationRow>, sqlx::Error> {
    if let Some(bot_id) = bot_id {
        sqlx::query_as(
            r#"
            SELECT c.id, c.owner_id, c.bot_id, c.conversation_type, c.name, c.created_at, c.updated_at
            FROM conversations c
            WHERE c.owner_id = $1
              AND (
                (c.conversation_type = 'direct' AND c.bot_id = $2 AND c.origin_kind = 'web')
                OR EXISTS (
                  SELECT 1 FROM conversation_participants p
                  WHERE p.conversation_id = c.id AND p.bot_id = $2 AND p.left_at IS NULL
                )
              )
            ORDER BY c.updated_at DESC
            LIMIT $3
            "#,
        )
        .bind(owner_id)
        .bind(bot_id)
        .bind(limit)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as(
            r#"
            SELECT id, owner_id, bot_id, conversation_type, name, created_at, updated_at
            FROM conversations
            WHERE owner_id = $1
              AND (conversation_type = 'group' OR origin_kind = 'web')
            ORDER BY updated_at DESC
            LIMIT $2
            "#,
        )
        .bind(owner_id)
        .bind(limit)
        .fetch_all(pool)
        .await
    }
}

pub fn normalize_engine_preference(raw: &str) -> Result<&'static str, ApiError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "auto" => Ok("auto"),
        "codex" => Ok("codex"),
        "responses" => Ok("responses"),
        _ => Err(ApiError::Validation(
            "enginePreference must be auto, codex, or responses".into(),
        )),
    }
}

pub fn normalize_model(raw: Option<&str>) -> String {
    canonical_bot_model(raw)
}

pub fn legacy_owner_for_bootstrap(owner_id: Option<&str>) -> &str {
    owner_id.unwrap_or(LEGACY_LOCAL_OWNER)
}
