//! Durable multi-bot group conversations and transcript authorship.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::db::queries::BootstrapRunRecords;
use crate::error::ApiError;
use crate::work;

pub const MIN_GROUP_BOTS: usize = 2;
pub const MAX_GROUP_BOTS: usize = 6;

fn db_error(error: sqlx::Error) -> ApiError {
    ApiError::Internal(error.to_string())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantSummary {
    pub bot_id: String,
    pub name: String,
    pub avatar_id: String,
    pub ordinal: i32,
    pub joined_at: DateTime<Utc>,
    pub left_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupConversationDetail {
    pub id: String,
    pub name: String,
    pub conversation_type: String,
    pub updated_at: DateTime<Utc>,
    pub participants: Vec<ParticipantSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptMessage {
    pub id: String,
    pub sequence: i64,
    pub role: String,
    pub body: String,
    pub status: String,
    pub author_kind: String,
    pub author_bot_id: Option<String>,
    pub author_bot_name: Option<String>,
    pub author_avatar_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub run_id: Option<String>,
}

pub async fn get_conversation_for_owner(
    pool: &PgPool,
    owner: &str,
    conversation_id: &str,
) -> Result<GroupConversationDetail, ApiError> {
    let row = sqlx::query(
        "SELECT id, name, conversation_type, updated_at FROM conversations WHERE id = $1 AND owner_id = $2",
    )
    .bind(conversation_id)
    .bind(owner)
    .fetch_optional(pool)
    .await
    .map_err(db_error)?
    .ok_or(ApiError::NotFound)?;

    let participants = load_participants(pool, owner, conversation_id).await?;
    Ok(GroupConversationDetail {
        id: row.get("id"),
        name: row
            .get::<Option<String>, _>("name")
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| "Group".into()),
        conversation_type: row.get("conversation_type"),
        updated_at: row.get("updated_at"),
        participants,
    })
}

async fn load_participants(
    pool: &PgPool,
    owner: &str,
    conversation_id: &str,
) -> Result<Vec<ParticipantSummary>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT p.bot_id, p.ordinal, p.joined_at, p.left_at, b.name, b.avatar_id
        FROM conversation_participants p
        JOIN bots b ON b.id = p.bot_id AND b.owner_id = p.owner_id
        WHERE p.conversation_id = $1 AND p.owner_id = $2
        ORDER BY p.ordinal ASC, p.joined_at ASC
        "#,
    )
    .bind(conversation_id)
    .bind(owner)
    .fetch_all(pool)
    .await
    .map_err(db_error)?;

    Ok(rows
        .into_iter()
        .map(|row| ParticipantSummary {
            bot_id: row.get("bot_id"),
            name: row.get("name"),
            avatar_id: row.get("avatar_id"),
            ordinal: row.get("ordinal"),
            joined_at: row.get("joined_at"),
            left_at: row.get("left_at"),
        })
        .collect())
}

pub async fn list_messages(
    pool: &PgPool,
    owner: &str,
    conversation_id: &str,
) -> Result<Vec<TranscriptMessage>, ApiError> {
    let _: () = get_conversation_for_owner(pool, owner, conversation_id)
        .await
        .map(|_| ())?;

    let rows = sqlx::query(
        r#"
        SELECT m.id, m.sequence, m.role, m.body, m.status, m.author_kind, m.author_bot_id,
               m.created_at, b.name AS author_bot_name, b.avatar_id AS author_avatar_id,
               (
                 SELECT r.id FROM agent_runs r
                 WHERE r.assistant_message_id = m.id
                 LIMIT 1
               ) AS run_id
        FROM messages m
        LEFT JOIN bots b ON b.id = m.author_bot_id
        WHERE m.conversation_id = $1
        ORDER BY m.sequence ASC
        "#,
    )
    .bind(conversation_id)
    .fetch_all(pool)
    .await
    .map_err(db_error)?;

    Ok(rows
        .into_iter()
        .map(|row| TranscriptMessage {
            id: row.get("id"),
            sequence: row.get("sequence"),
            role: row.get("role"),
            body: row.get("body"),
            status: row.get("status"),
            author_kind: row.get("author_kind"),
            author_bot_id: row.get("author_bot_id"),
            author_bot_name: row.get("author_bot_name"),
            author_avatar_id: row.get("author_avatar_id"),
            created_at: row.get("created_at"),
            run_id: row.get("run_id"),
        })
        .collect())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGroupRequest {
    pub name: String,
    pub bot_ids: Vec<String>,
}

pub async fn create_group(
    pool: &PgPool,
    owner: &str,
    body: CreateGroupRequest,
) -> Result<GroupConversationDetail, ApiError> {
    let name = body.name.trim();
    if name.is_empty() || name.len() > 200 {
        return Err(ApiError::Validation(
            "name must be between 1 and 200 characters".into(),
        ));
    }
    let bot_ids = normalize_group_bot_ids(&body.bot_ids)?;
    validate_group_bots(pool, owner, &bot_ids).await?;

    let conversation_id = Uuid::new_v4().to_string();
    let mut tx = pool.begin().await.map_err(db_error)?;
    sqlx::query(
        "INSERT INTO conversations (id, owner_id, conversation_type, name) VALUES ($1, $2, 'group', $3)",
    )
    .bind(&conversation_id)
    .bind(owner)
    .bind(name)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;

    for (ordinal, bot_id) in bot_ids.iter().enumerate() {
        sqlx::query(
            r#"
            INSERT INTO conversation_participants (conversation_id, bot_id, owner_id, ordinal)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(&conversation_id)
        .bind(bot_id)
        .bind(owner)
        .bind(ordinal as i32)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    }
    tx.commit().await.map_err(db_error)?;

    get_conversation_for_owner(pool, owner, &conversation_id).await
}

fn normalize_group_bot_ids(bot_ids: &[String]) -> Result<Vec<String>, ApiError> {
    let mut unique = Vec::new();
    for raw in bot_ids {
        let id = raw.trim();
        if id.is_empty() {
            continue;
        }
        if unique.iter().any(|existing| existing == id) {
            return Err(ApiError::Validation("duplicate botIds".into()));
        }
        unique.push(id.to_string());
    }
    if unique.len() < MIN_GROUP_BOTS {
        return Err(ApiError::Validation(format!(
            "at least {} bots are required for a group",
            MIN_GROUP_BOTS
        )));
    }
    if unique.len() > MAX_GROUP_BOTS {
        return Err(ApiError::Validation(format!(
            "at most {} bots are allowed in a group",
            MAX_GROUP_BOTS
        )));
    }
    Ok(unique)
}

async fn validate_group_bots(
    pool: &PgPool,
    owner: &str,
    bot_ids: &[String],
) -> Result<(), ApiError> {
    for bot_id in bot_ids {
        let ok: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
              SELECT 1 FROM bots b
              JOIN sandboxes s ON s.id = b.computer_id AND s.owner_id = b.owner_id
              WHERE b.id = $1 AND b.owner_id = $2 AND s.state <> 'archived'
            )
            "#,
        )
        .bind(bot_id)
        .bind(owner)
        .fetch_one(pool)
        .await
        .map_err(db_error)?;
        if !ok {
            return Err(ApiError::NotFound);
        }
    }
    Ok(())
}

pub async fn add_participant(
    pool: &PgPool,
    owner: &str,
    conversation_id: &str,
    bot_id: &str,
) -> Result<GroupConversationDetail, ApiError> {
    let detail = get_conversation_for_owner(pool, owner, conversation_id).await?;
    if detail.conversation_type != "group" {
        return Err(ApiError::Validation("not a group conversation".into()));
    }
    let active = detail
        .participants
        .iter()
        .filter(|p| p.left_at.is_none())
        .count();
    if active >= MAX_GROUP_BOTS {
        return Err(ApiError::Validation(format!(
            "at most {} active bots are allowed in a group",
            MAX_GROUP_BOTS
        )));
    }
    validate_group_bots(pool, owner, &[bot_id.to_string()]).await?;

    let ordinal: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(ordinal), -1) + 1 FROM conversation_participants WHERE conversation_id = $1",
    )
    .bind(conversation_id)
    .fetch_one(pool)
    .await
    .map_err(db_error)?;

    sqlx::query(
        r#"
        INSERT INTO conversation_participants (conversation_id, bot_id, owner_id, ordinal, left_at)
        VALUES ($1, $2, $3, $4, NULL)
        ON CONFLICT (conversation_id, bot_id) DO UPDATE
        SET left_at = NULL, ordinal = EXCLUDED.ordinal, joined_at = NOW()
        "#,
    )
    .bind(conversation_id)
    .bind(bot_id)
    .bind(owner)
    .bind(ordinal)
    .execute(pool)
    .await
    .map_err(db_error)?;

    sqlx::query("UPDATE conversations SET updated_at = NOW() WHERE id = $1")
        .bind(conversation_id)
        .execute(pool)
        .await
        .map_err(db_error)?;

    get_conversation_for_owner(pool, owner, conversation_id).await
}

pub async fn remove_participant(
    pool: &PgPool,
    owner: &str,
    conversation_id: &str,
    bot_id: &str,
) -> Result<GroupConversationDetail, ApiError> {
    let detail = get_conversation_for_owner(pool, owner, conversation_id).await?;
    if detail.conversation_type != "group" {
        return Err(ApiError::Validation("not a group conversation".into()));
    }
    let active = detail
        .participants
        .iter()
        .filter(|p| p.left_at.is_none())
        .count();
    if active <= MIN_GROUP_BOTS {
        return Err(ApiError::Validation(format!(
            "at least {} active bots must remain in a group",
            MIN_GROUP_BOTS
        )));
    }
    let updated = sqlx::query(
        r#"
        UPDATE conversation_participants
        SET left_at = NOW()
        WHERE conversation_id = $1 AND bot_id = $2 AND owner_id = $3 AND left_at IS NULL
        "#,
    )
    .bind(conversation_id)
    .bind(bot_id)
    .bind(owner)
    .execute(pool)
    .await
    .map_err(db_error)?;
    if updated.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    get_conversation_for_owner(pool, owner, conversation_id).await
}

pub async fn append_human_message(
    pool: &PgPool,
    owner: &str,
    conversation_id: &str,
    body: &str,
) -> Result<TranscriptMessage, ApiError> {
    let detail = get_conversation_for_owner(pool, owner, conversation_id).await?;
    if detail.conversation_type != "group" {
        return Err(ApiError::Validation(
            "human-only messages require a group conversation".into(),
        ));
    }
    let trimmed = body.trim();
    if trimmed.is_empty() || trimmed.len() > 100_000 {
        return Err(ApiError::Validation(
            "message must be between 1 and 100,000 bytes".into(),
        ));
    }

    let message_id = Uuid::new_v4().to_string();
    let mut tx = pool.begin().await.map_err(db_error)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(format!("conversation-seq:{conversation_id}"))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    let sequence: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sequence), 0) + 1 FROM messages WHERE conversation_id = $1",
    )
    .bind(conversation_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(db_error)?;
    sqlx::query(
        r#"
        INSERT INTO messages (id, conversation_id, role, body, status, sequence, author_kind)
        VALUES ($1, $2, 'user', $3, 'complete', $4, 'human')
        "#,
    )
    .bind(&message_id)
    .bind(conversation_id)
    .bind(trimmed)
    .bind(sequence)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    sqlx::query("UPDATE conversations SET updated_at = NOW() WHERE id = $1")
        .bind(conversation_id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;

    Ok(TranscriptMessage {
        id: message_id,
        sequence,
        role: "user".into(),
        body: trimmed.to_string(),
        status: "complete".into(),
        author_kind: "human".into(),
        author_bot_id: None,
        author_bot_name: None,
        author_avatar_id: None,
        created_at: Utc::now(),
        run_id: None,
    })
}

pub async fn is_active_participant(
    pool: &PgPool,
    owner: &str,
    conversation_id: &str,
    bot_id: &str,
) -> Result<bool, ApiError> {
    let ok: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
          SELECT 1 FROM conversation_participants p
          JOIN conversations c ON c.id = p.conversation_id
          WHERE p.conversation_id = $1 AND p.bot_id = $2 AND p.owner_id = $3
            AND p.left_at IS NULL AND c.conversation_type = 'group'
        )
        "#,
    )
    .bind(conversation_id)
    .bind(bot_id)
    .bind(owner)
    .fetch_one(pool)
    .await
    .map_err(db_error)?;
    Ok(ok)
}

pub async fn enqueue_group_bot_run(
    pool: &PgPool,
    owner: &str,
    conversation_id: &str,
    bot_id: &str,
    request_id: &str,
    message: &str,
) -> Result<BootstrapRunRecords, ApiError> {
    if !is_active_participant(pool, owner, conversation_id, bot_id).await? {
        return Err(ApiError::NotFound);
    }
    work::enqueue(
        pool,
        owner,
        request_id,
        bot_id,
        Some(conversation_id),
        message,
    )
    .await
}

pub async fn assert_bot_may_use_conversation(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &str,
    bot_id: &str,
    conversation_id: &str,
) -> Result<(), ApiError> {
    let row = sqlx::query(
        "SELECT conversation_type, bot_id FROM conversations WHERE id = $1 AND owner_id = $2",
    )
    .bind(conversation_id)
    .bind(owner)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    .ok_or(ApiError::NotFound)?;

    let conversation_type: String = row.get("conversation_type");
    if conversation_type == "direct" {
        let conv_bot: Option<String> = row.get("bot_id");
        if conv_bot.as_deref() != Some(bot_id) {
            return Err(ApiError::NotFound);
        }
        return Ok(());
    }
    if conversation_type == "group" {
        let active: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
              SELECT 1 FROM conversation_participants
              WHERE conversation_id = $1 AND bot_id = $2 AND owner_id = $3 AND left_at IS NULL
            )
            "#,
        )
        .bind(conversation_id)
        .bind(bot_id)
        .bind(owner)
        .fetch_one(&mut **tx)
        .await
        .map_err(db_error)?;
        if !active {
            return Err(ApiError::NotFound);
        }
        return Ok(());
    }
    Err(ApiError::Internal("unknown conversation type".into()))
}
