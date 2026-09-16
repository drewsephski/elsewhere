//! Durable multi-bot group conversations and transcript authorship.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::db::queries::BootstrapRunRecords;
use crate::error::ApiError;
use crate::group_router::{
    cancel_routing_for_deleted_message, resolve_group_routing_mode, GroupRoutingMode,
};
use crate::skills::SkillAdmissionInput;
use crate::work;

pub const MIN_GROUP_BOTS: usize = 2;
pub const MAX_GROUP_BOTS: usize = 6;

fn db_error(error: sqlx::Error) -> ApiError {
    ApiError::Internal(error.to_string())
}

/// Stable fingerprint for idempotent group sends (same key must replay the same operation).
pub fn group_send_request_fingerprint(
    body: &str,
    routing_mode: &str,
    recipient_bot_ids: Option<&[String]>,
    attachment_ids: &[String],
) -> String {
    let mut recipients: Vec<String> = recipient_bot_ids
        .map(|ids| {
            ids.iter()
                .map(|id| id.trim().to_string())
                .filter(|id| !id.is_empty())
                .collect()
        })
        .unwrap_or_default();
    recipients.sort();
    recipients.dedup();
    let mode = routing_mode.trim().to_ascii_lowercase();
    let mut fingerprint = format!("v2|{}|{}|{}", body.trim(), mode, recipients.join(","));
    if !attachment_ids.is_empty() {
        fingerprint.push_str("|att:");
        fingerprint.push_str(&attachment_ids.join(","));
    }
    fingerprint
}

async fn load_idempotent_group_send(
    pool: &PgPool,
    owner: &str,
    conversation_id: &str,
    idempotency_key: &str,
    fingerprint: &str,
) -> Result<Option<SendGroupMessageResponse>, ApiError> {
    let existing = sqlx::query(
        r#"
        SELECT message_id, request_fingerprint
        FROM group_message_sends
        WHERE owner_id = $1 AND conversation_id = $2 AND idempotency_key = $3
        "#,
    )
    .bind(owner)
    .bind(conversation_id)
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    let Some(existing) = existing else {
        return Ok(None);
    };
    let stored: String = existing.get("request_fingerprint");
    if stored != fingerprint {
        return Err(ApiError::Conflict(
            "Idempotency-Key was already used with a different request payload".into(),
        ));
    }
    let message_id: String = existing.get("message_id");
    let messages = list_messages(pool, owner, conversation_id).await?;
    let message = messages
        .into_iter()
        .find(|m| m.id == message_id)
        .ok_or(ApiError::Internal("idempotent send missing message".into()))?;
    let recipients = message.recipients.clone().unwrap_or_default();
    Ok(Some(SendGroupMessageResponse {
        message,
        recipients,
    }))
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
pub struct MessageRouting {
    pub mode: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageRecipient {
    pub bot_id: String,
    pub bot_name: String,
    pub avatar_id: String,
    pub routing_kind: String,
    pub status: String,
    pub run_id: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipients: Option<Vec<MessageRecipient>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing: Option<MessageRouting>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub attachments: Vec<agent_core::AttachmentDescriptor>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupListItem {
    pub id: String,
    pub name: String,
    pub updated_at: DateTime<Utc>,
    pub participants: Vec<ParticipantSummary>,
    pub queued_runs: i64,
    pub working_runs: i64,
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
          AND m.deleted_at IS NULL
          AND m.kind = $2
        ORDER BY m.sequence ASC
        "#,
    )
    .bind(conversation_id)
    .bind(crate::message_kind::CHAT_MESSAGE_KIND)
    .fetch_all(pool)
    .await
    .map_err(db_error)?;

    let messages: Vec<TranscriptMessage> = rows
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
            recipients: None,
            routing: None,
            attachments: Vec::new(),
        })
        .collect();

    let human_ids: Vec<String> = messages
        .iter()
        .filter(|m| m.author_kind == "human")
        .map(|m| m.id.clone())
        .collect();
    if human_ids.is_empty() {
        return Ok(messages);
    }

    let send_rows = sqlx::query(
        r#"
        SELECT message_id, routing_mode, routing_status, routing_error
        FROM group_message_sends
        WHERE conversation_id = $1 AND message_id = ANY($2)
        "#,
    )
    .bind(conversation_id)
    .bind(&human_ids)
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    let mut routing_by_message: std::collections::HashMap<String, MessageRouting> =
        std::collections::HashMap::new();
    for row in send_rows {
        routing_by_message.insert(
            row.get("message_id"),
            MessageRouting {
                mode: row.get("routing_mode"),
                status: row.get("routing_status"),
                error_code: row.get("routing_error"),
            },
        );
    }

    let recipient_rows = sqlx::query(
        r#"
        SELECT g.message_id, g.bot_id, g.routing_kind, g.status, g.run_id, b.name, b.avatar_id
        FROM group_message_recipients g
        JOIN bots b ON b.id = g.bot_id
        WHERE g.conversation_id = $1 AND g.message_id = ANY($2)
        ORDER BY g.created_at ASC
        "#,
    )
    .bind(conversation_id)
    .bind(&human_ids)
    .fetch_all(pool)
    .await
    .map_err(db_error)?;

    let mut by_message: std::collections::HashMap<String, Vec<MessageRecipient>> =
        std::collections::HashMap::new();
    for row in recipient_rows {
        let message_id: String = row.get("message_id");
        by_message
            .entry(message_id)
            .or_default()
            .push(MessageRecipient {
                bot_id: row.get("bot_id"),
                bot_name: row.get("name"),
                avatar_id: row.get("avatar_id"),
                routing_kind: row.get("routing_kind"),
                status: row.get("status"),
                run_id: row.get("run_id"),
            });
    }

    let mut messages: Vec<TranscriptMessage> = messages
        .into_iter()
        .map(|mut m| {
            if m.author_kind == "human" {
                m.recipients = by_message.get(&m.id).cloned();
                m.routing = routing_by_message.get(&m.id).cloned();
            }
            m
        })
        .collect();
    let attachment_ids: Vec<String> = messages.iter().map(|m| m.id.clone()).collect();
    let attached = crate::attachments::store::list_for_messages(pool, &attachment_ids).await?;
    for message in &mut messages {
        if let Some(items) = attached.get(&message.id) {
            message.attachments = items.clone();
        }
    }
    Ok(messages)
}

pub async fn list_groups(
    pool: &PgPool,
    owner: &str,
    limit: i64,
) -> Result<Vec<GroupListItem>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT c.id, c.name, c.updated_at
        FROM conversations c
        WHERE c.owner_id = $1 AND c.conversation_type = 'group'
        ORDER BY c.updated_at DESC
        LIMIT $2
        "#,
    )
    .bind(owner)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(db_error)?;

    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let group_ids: Vec<String> = rows.iter().map(|row| row.get("id")).collect();

    let participant_rows = sqlx::query(
        r#"
        SELECT p.conversation_id, p.bot_id, p.ordinal, p.joined_at, p.left_at, b.name, b.avatar_id
        FROM conversation_participants p
        JOIN bots b ON b.id = p.bot_id AND b.owner_id = p.owner_id
        WHERE p.owner_id = $1 AND p.conversation_id = ANY($2)
        ORDER BY p.conversation_id, p.ordinal ASC, p.joined_at ASC
        "#,
    )
    .bind(owner)
    .bind(&group_ids)
    .fetch_all(pool)
    .await
    .map_err(db_error)?;

    let mut participants_by_group: std::collections::HashMap<String, Vec<ParticipantSummary>> =
        std::collections::HashMap::new();
    for row in participant_rows {
        let conversation_id: String = row.get("conversation_id");
        participants_by_group
            .entry(conversation_id)
            .or_default()
            .push(ParticipantSummary {
                bot_id: row.get("bot_id"),
                name: row.get("name"),
                avatar_id: row.get("avatar_id"),
                ordinal: row.get("ordinal"),
                joined_at: row.get("joined_at"),
                left_at: row.get("left_at"),
            });
    }

    let run_count_rows = sqlx::query(
        r#"
        SELECT g.conversation_id,
               COUNT(*) FILTER (WHERE r.status = 'queued') AS queued_runs,
               COUNT(*) FILTER (WHERE r.status = 'running') AS working_runs
        FROM group_message_recipients g
        JOIN agent_runs r ON r.id = g.run_id
        WHERE g.conversation_id = ANY($1)
        GROUP BY g.conversation_id
        "#,
    )
    .bind(&group_ids)
    .fetch_all(pool)
    .await
    .map_err(db_error)?;

    let mut queued_by_group: std::collections::HashMap<String, i64> =
        std::collections::HashMap::new();
    let mut working_by_group: std::collections::HashMap<String, i64> =
        std::collections::HashMap::new();
    for row in run_count_rows {
        let id: String = row.get("conversation_id");
        queued_by_group.insert(id.clone(), row.get("queued_runs"));
        working_by_group.insert(id, row.get("working_runs"));
    }

    let out = rows
        .into_iter()
        .map(|row| {
            let id: String = row.get("id");
            GroupListItem {
                id: id.clone(),
                name: row
                    .get::<Option<String>, _>("name")
                    .filter(|n| !n.trim().is_empty())
                    .unwrap_or_else(|| "Group".into()),
                updated_at: row.get("updated_at"),
                participants: participants_by_group.remove(&id).unwrap_or_default(),
                queued_runs: queued_by_group.get(&id).copied().unwrap_or(0),
                working_runs: working_by_group.get(&id).copied().unwrap_or(0),
            }
        })
        .collect();

    Ok(out)
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
    let name = validate_group_name(&body.name)?;
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

async fn require_group_conversation(
    pool: &PgPool,
    owner: &str,
    conversation_id: &str,
) -> Result<GroupConversationDetail, ApiError> {
    let detail = get_conversation_for_owner(pool, owner, conversation_id).await?;
    if detail.conversation_type != "group" {
        return Err(ApiError::Validation("not a group conversation".into()));
    }
    Ok(detail)
}

fn validate_group_name(name: &str) -> Result<&str, ApiError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.len() > 200 {
        return Err(ApiError::Validation(
            "name must be between 1 and 200 characters".into(),
        ));
    }
    Ok(trimmed)
}

pub async fn rename_group(
    pool: &PgPool,
    owner: &str,
    conversation_id: &str,
    name: &str,
) -> Result<GroupConversationDetail, ApiError> {
    let name = validate_group_name(name)?;
    require_group_conversation(pool, owner, conversation_id).await?;
    let updated = sqlx::query(
        r#"
        UPDATE conversations
        SET name = $3, updated_at = NOW()
        WHERE id = $1 AND owner_id = $2 AND conversation_type = 'group'
        "#,
    )
    .bind(conversation_id)
    .bind(owner)
    .bind(name)
    .execute(pool)
    .await
    .map_err(db_error)?;
    if updated.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    get_conversation_for_owner(pool, owner, conversation_id).await
}

pub async fn delete_group(
    pool: &PgPool,
    owner: &str,
    conversation_id: &str,
) -> Result<(), ApiError> {
    require_group_conversation(pool, owner, conversation_id).await?;

    sqlx::query(
        r#"
        UPDATE group_message_sends
        SET routing_status = 'cancelled',
            routing_claim_token = NULL,
            routing_lease_until = NULL
        WHERE conversation_id = $1
          AND routing_mode = 'auto'
          AND routing_status IN ('pending', 'routing')
        "#,
    )
    .bind(conversation_id)
    .execute(pool)
    .await
    .map_err(db_error)?;

    let mut tx = pool.begin().await.map_err(db_error)?;

    sqlx::query(
        "UPDATE routines SET destination_conversation_id = NULL WHERE destination_conversation_id = $1",
    )
    .bind(conversation_id)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;

    sqlx::query(
        r#"
        UPDATE routines
        SET last_run_id = NULL
        WHERE last_run_id IN (SELECT id FROM agent_runs WHERE conversation_id = $1 AND owner_id = $2)
        "#,
    )
    .bind(conversation_id)
    .bind(owner)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;

    sqlx::query(
        r#"
        UPDATE routines
        SET acknowledged_run_id = NULL
        WHERE acknowledged_run_id IN (SELECT id FROM agent_runs WHERE conversation_id = $1 AND owner_id = $2)
        "#,
    )
    .bind(conversation_id)
    .bind(owner)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;

    sqlx::query(
        r#"
        UPDATE routine_runs
        SET run_id = NULL
        WHERE run_id IN (SELECT id FROM agent_runs WHERE conversation_id = $1 AND owner_id = $2)
        "#,
    )
    .bind(conversation_id)
    .bind(owner)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;

    sqlx::query(
        r#"
        DELETE FROM work_queue
        WHERE run_id IN (SELECT id FROM agent_runs WHERE conversation_id = $1 AND owner_id = $2)
           OR delegation_id IN (
                SELECT id FROM bot_delegations
                WHERE owner_id = $2
                  AND (
                    source_conversation_id = $1
                    OR target_conversation_id = $1
                    OR source_run_id IN (SELECT id FROM agent_runs WHERE conversation_id = $1 AND owner_id = $2)
                    OR target_run_id IN (SELECT id FROM agent_runs WHERE conversation_id = $1 AND owner_id = $2)
                    OR root_run_id IN (SELECT id FROM agent_runs WHERE conversation_id = $1 AND owner_id = $2)
                    OR source_resume_run_id IN (SELECT id FROM agent_runs WHERE conversation_id = $1 AND owner_id = $2)
                  )
           )
        "#,
    )
    .bind(conversation_id)
    .bind(owner)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;

    sqlx::query(
        r#"
        DELETE FROM tool_approval_requests
        WHERE run_id IN (SELECT id FROM agent_runs WHERE conversation_id = $1 AND owner_id = $2)
        "#,
    )
    .bind(conversation_id)
    .bind(owner)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;

    sqlx::query(
        r#"
        DELETE FROM work_results
        WHERE run_id IN (SELECT id FROM agent_runs WHERE conversation_id = $1 AND owner_id = $2)
        "#,
    )
    .bind(conversation_id)
    .bind(owner)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;

    sqlx::query(
        r#"
        DELETE FROM run_events
        WHERE request_id IN (SELECT request_id FROM agent_runs WHERE conversation_id = $1 AND owner_id = $2)
        "#,
    )
    .bind(conversation_id)
    .bind(owner)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;

    sqlx::query(
        r#"
        UPDATE bot_delegations
        SET parent_delegation_id = NULL
        WHERE parent_delegation_id IN (
            SELECT id FROM (
                SELECT d.id
                FROM bot_delegations d
                WHERE d.owner_id = $2
                  AND (
                    d.source_conversation_id = $1
                    OR d.target_conversation_id = $1
                    OR d.source_run_id IN (SELECT id FROM agent_runs WHERE conversation_id = $1 AND owner_id = $2)
                    OR d.target_run_id IN (SELECT id FROM agent_runs WHERE conversation_id = $1 AND owner_id = $2)
                    OR d.root_run_id IN (SELECT id FROM agent_runs WHERE conversation_id = $1 AND owner_id = $2)
                    OR d.source_resume_run_id IN (SELECT id FROM agent_runs WHERE conversation_id = $1 AND owner_id = $2)
                  )
            ) matching
        )
        "#,
    )
    .bind(conversation_id)
    .bind(owner)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;

    sqlx::query(
        r#"
        DELETE FROM bot_delegations
        WHERE owner_id = $2
          AND (
            source_conversation_id = $1
            OR target_conversation_id = $1
            OR source_run_id IN (SELECT id FROM agent_runs WHERE conversation_id = $1 AND owner_id = $2)
            OR target_run_id IN (SELECT id FROM agent_runs WHERE conversation_id = $1 AND owner_id = $2)
            OR root_run_id IN (SELECT id FROM agent_runs WHERE conversation_id = $1 AND owner_id = $2)
            OR source_resume_run_id IN (SELECT id FROM agent_runs WHERE conversation_id = $1 AND owner_id = $2)
          )
        "#,
    )
    .bind(conversation_id)
    .bind(owner)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;

    sqlx::query("DELETE FROM agent_runs WHERE conversation_id = $1 AND owner_id = $2")
        .bind(conversation_id)
        .bind(owner)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

    sqlx::query("DELETE FROM messages WHERE conversation_id = $1")
        .bind(conversation_id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

    let deleted = sqlx::query(
        "DELETE FROM conversations WHERE id = $1 AND owner_id = $2 AND conversation_type = 'group'",
    )
    .bind(conversation_id)
    .bind(owner)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    if deleted.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }

    tx.commit().await.map_err(db_error)?;
    Ok(())
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
    validate_group_bots(pool, owner, &[bot_id.to_string()]).await?;

    let mut tx = pool.begin().await.map_err(db_error)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(format!("group-members:{conversation_id}"))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

    let active: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM conversation_participants
        WHERE conversation_id = $1 AND owner_id = $2 AND left_at IS NULL
        "#,
    )
    .bind(conversation_id)
    .bind(owner)
    .fetch_one(&mut *tx)
    .await
    .map_err(db_error)?;
    if active as usize >= MAX_GROUP_BOTS {
        return Err(ApiError::Validation(format!(
            "at most {} active bots are allowed in a group",
            MAX_GROUP_BOTS
        )));
    }

    let ordinal: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(ordinal), -1) + 1 FROM conversation_participants WHERE conversation_id = $1",
    )
    .bind(conversation_id)
    .fetch_one(&mut *tx)
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
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;

    sqlx::query("UPDATE conversations SET updated_at = NOW() WHERE id = $1")
        .bind(conversation_id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;

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

    let mut tx = pool.begin().await.map_err(db_error)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(format!("group-members:{conversation_id}"))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

    let active: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM conversation_participants
        WHERE conversation_id = $1 AND owner_id = $2 AND left_at IS NULL
        "#,
    )
    .bind(conversation_id)
    .bind(owner)
    .fetch_one(&mut *tx)
    .await
    .map_err(db_error)?;
    if active as usize <= MIN_GROUP_BOTS {
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
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    if updated.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    tx.commit().await.map_err(db_error)?;
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
        recipients: None,
        routing: None,
        attachments: Vec::new(),
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
    let trimmed = message.trim();
    let mut tx = pool.begin().await.map_err(db_error)?;
    let message_id = Uuid::new_v4().to_string();
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
    let records = work::enqueue_from_group_message_in_transaction(
        &mut tx,
        owner,
        request_id,
        bot_id,
        conversation_id,
        &message_id,
        trimmed,
        &SkillAdmissionInput::default(),
    )
    .await?;
    tx.commit().await.map_err(db_error)?;
    Ok(records)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendGroupMessageRequest {
    pub body: String,
    pub recipient_bot_ids: Option<Vec<String>>,
    pub mention_mode: Option<String>,
    pub routing_mode: Option<String>,
    pub skill_invocation: Option<serde_json::Value>,
    #[serde(default)]
    pub attachment_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendGroupMessageResponse {
    pub message: TranscriptMessage,
    pub recipients: Vec<MessageRecipient>,
}

fn resolve_group_recipients(
    mention_mode: Option<&str>,
    recipient_bot_ids: Option<&[String]>,
    active_bot_ids: &[String],
) -> Result<Vec<(String, String)>, ApiError> {
    let everyone = mention_mode
        .map(|m| m.eq_ignore_ascii_case("everyone"))
        .unwrap_or(false);
    let mut resolved: Vec<String> = Vec::new();
    if everyone {
        resolved.extend(active_bot_ids.iter().cloned());
    }
    if let Some(ids) = recipient_bot_ids {
        for raw in ids {
            let id = raw.trim();
            if id.is_empty() {
                continue;
            }
            if !resolved.iter().any(|existing| existing == id) {
                resolved.push(id.to_string());
            }
        }
    }
    if resolved.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for bot_id in resolved {
        if !active_bot_ids.iter().any(|active| active == &bot_id) {
            return Err(ApiError::NotFound);
        }
        let kind = if everyone && active_bot_ids.contains(&bot_id) {
            "everyone"
        } else {
            "mention"
        };
        out.push((bot_id, kind.to_string()));
    }
    Ok(out)
}

pub async fn send_group_message(
    pool: &PgPool,
    owner: &str,
    conversation_id: &str,
    idempotency_key: &str,
    body: SendGroupMessageRequest,
) -> Result<SendGroupMessageResponse, ApiError> {
    if idempotency_key.len() > 200 {
        return Err(ApiError::Validation("Idempotency key is too long".into()));
    }
    let detail = get_conversation_for_owner(pool, owner, conversation_id).await?;
    if detail.conversation_type != "group" {
        return Err(ApiError::Validation("not a group conversation".into()));
    }
    let trimmed = body.body.trim();
    if (trimmed.is_empty() && body.attachment_ids.is_empty()) || trimmed.len() > 100_000 {
        return Err(ApiError::Validation(
            "message must be between 1 and 100,000 bytes".into(),
        ));
    }

    let active_bot_ids: Vec<String> = detail
        .participants
        .iter()
        .filter(|p| p.left_at.is_none())
        .map(|p| p.bot_id.clone())
        .collect();

    for raw in body.recipient_bot_ids.as_deref().unwrap_or(&[]) {
        let id = raw.trim();
        if id.is_empty() {
            continue;
        }
        let owned: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM bots WHERE id = $1 AND owner_id = $2)")
                .bind(id)
                .bind(owner)
                .fetch_one(pool)
                .await
                .map_err(db_error)?;
        if !owned {
            return Err(ApiError::NotFound);
        }
    }

    let routing_mode = resolve_group_routing_mode(
        body.routing_mode.as_deref(),
        body.mention_mode.as_deref(),
        body.recipient_bot_ids.as_deref(),
    );

    let explicit_recipients = if routing_mode == GroupRoutingMode::Auto {
        Vec::new()
    } else {
        resolve_group_recipients(
            if routing_mode == GroupRoutingMode::Everyone {
                Some("everyone")
            } else {
                None
            },
            body.recipient_bot_ids.as_deref(),
            &active_bot_ids,
        )?
    };

    let fingerprint = group_send_request_fingerprint(
        trimmed,
        routing_mode.as_str(),
        body.recipient_bot_ids.as_deref(),
        &body.attachment_ids,
    );

    if let Some(replay) =
        load_idempotent_group_send(pool, owner, conversation_id, idempotency_key, &fingerprint)
            .await?
    {
        return Ok(replay);
    }

    let send_id = Uuid::new_v4().to_string();
    let message_id = Uuid::new_v4().to_string();
    let mut tx = pool.begin().await.map_err(db_error)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(format!("group-send:{owner}:{idempotency_key}"))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

    if let Some(replay) =
        load_idempotent_group_send(pool, owner, conversation_id, idempotency_key, &fingerprint)
            .await?
    {
        tx.commit().await.map_err(db_error)?;
        return Ok(replay);
    }

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
        INSERT INTO messages (id, conversation_id, role, body, status, sequence, author_kind, skill_invocation)
        VALUES ($1, $2, 'user', $3, 'complete', $4, 'human', $5)
        "#,
    )
    .bind(&message_id)
    .bind(conversation_id)
    .bind(trimmed)
    .bind(sequence)
    .bind(body.skill_invocation.as_ref())
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;

    if !body.attachment_ids.is_empty() {
        let staged = crate::attachments::store::load_staged_for_admission(
            &mut tx,
            owner,
            None,
            Some(conversation_id),
            &body.attachment_ids,
        )
        .await?;
        crate::attachments::store::attach_to_message_only(&mut tx, &message_id, &staged).await?;
    }

    let candidate_fingerprint = if routing_mode == GroupRoutingMode::Auto {
        Some(crate::group_router::candidate_fingerprint(
            &crate::group_router::load_route_candidates(pool, owner, &detail).await?,
        ))
    } else {
        None
    };

    sqlx::query(
        r#"
        INSERT INTO group_message_sends (
            id, owner_id, conversation_id, idempotency_key, message_id, request_fingerprint,
            routing_mode, routing_status, routed_at, routing_candidate_fingerprint
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#,
    )
    .bind(&send_id)
    .bind(owner)
    .bind(conversation_id)
    .bind(idempotency_key)
    .bind(&message_id)
    .bind(&fingerprint)
    .bind(routing_mode.as_str())
    .bind(if routing_mode == GroupRoutingMode::Auto {
        "pending"
    } else {
        "resolved"
    })
    .bind(if routing_mode == GroupRoutingMode::Auto {
        None::<DateTime<Utc>>
    } else {
        Some(Utc::now())
    })
    .bind(candidate_fingerprint.as_deref())
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;

    let mut recipients_out = Vec::new();
    for (bot_id, routing_kind) in explicit_recipients {
        let run_request_id = format!("{conversation_id}:{idempotency_key}:{bot_id}");
        let records = work::enqueue_from_group_message_in_transaction(
            &mut tx,
            owner,
            &run_request_id,
            &bot_id,
            conversation_id,
            &message_id,
            trimmed,
            &SkillAdmissionInput::default(),
        )
        .await?;
        crate::attachments::store::snapshot_message_attachments_onto_run(
            &mut tx,
            &message_id,
            &records.run_id,
        )
        .await?;
        sqlx::query(
            r#"
            INSERT INTO group_message_recipients (message_id, conversation_id, bot_id, run_id, routing_kind, status)
            VALUES ($1, $2, $3, $4, $5, 'queued')
            ON CONFLICT (message_id, bot_id) DO NOTHING
            "#,
        )
        .bind(&message_id)
        .bind(conversation_id)
        .bind(&bot_id)
        .bind(&records.run_id)
        .bind(&routing_kind)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        let participant = detail
            .participants
            .iter()
            .find(|p| p.bot_id == bot_id)
            .ok_or(ApiError::NotFound)?;
        recipients_out.push(MessageRecipient {
            bot_id: bot_id.clone(),
            bot_name: participant.name.clone(),
            avatar_id: participant.avatar_id.clone(),
            routing_kind,
            status: "queued".into(),
            run_id: Some(records.run_id),
        });
    }

    sqlx::query("UPDATE conversations SET updated_at = NOW() WHERE id = $1")
        .bind(conversation_id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;

    Ok(SendGroupMessageResponse {
        message: TranscriptMessage {
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
            recipients: Some(recipients_out.clone()),
            routing: Some(MessageRouting {
                mode: routing_mode.as_str().to_string(),
                status: if routing_mode == GroupRoutingMode::Auto {
                    "pending".into()
                } else {
                    "resolved".into()
                },
                error_code: None,
            }),
            attachments: Vec::new(),
        },
        recipients: recipients_out,
    })
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

pub async fn delete_transcript_message(
    state: &crate::app_state::AppState,
    owner: &str,
    conversation_id: &str,
    message_id: &str,
) -> Result<(), ApiError> {
    get_conversation_for_owner(&state.pool, owner, conversation_id).await?;

    let exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
          SELECT 1 FROM messages m
          JOIN conversations c ON c.id = m.conversation_id
          WHERE m.id = $1 AND m.conversation_id = $2 AND c.owner_id = $3 AND m.deleted_at IS NULL
        )
        "#,
    )
    .bind(message_id)
    .bind(conversation_id)
    .bind(owner)
    .fetch_one(&state.pool)
    .await
    .map_err(db_error)?;
    if !exists {
        return Err(ApiError::NotFound);
    }

    cancel_routing_for_deleted_message(&state.pool, message_id).await?;

    let run_ids: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT run_id FROM (
          SELECT g.run_id AS run_id
          FROM group_message_recipients g
          WHERE g.message_id = $1 AND g.run_id IS NOT NULL
          UNION
          SELECT r.id AS run_id
          FROM agent_runs r
          WHERE r.source_message_id = $1 OR r.assistant_message_id = $1
        ) AS linked
        WHERE run_id IS NOT NULL
        "#,
    )
    .bind(message_id)
    .fetch_all(&state.pool)
    .await
    .map_err(db_error)?;

    for run_id in run_ids {
        crate::run_archive::archive_run(state, owner, &run_id).await?;
    }

    sqlx::query(
        "UPDATE messages SET deleted_at = NOW(), updated_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(message_id)
    .execute(&state.pool)
    .await
    .map_err(db_error)?;

    Ok(())
}
