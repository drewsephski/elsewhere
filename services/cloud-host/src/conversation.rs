//! Primary conversation resolution and bounded chat history for multi-turn runs.
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::ApiError;

/// Max user+assistant messages passed to the Responses API (excluding the in-flight turn).
pub const MAX_RESPONSES_HISTORY_MESSAGES: usize = 80;

/// After this many completed assistant turns in a conversation, trigger Codex compaction.
pub const CODEX_COMPACT_COMPLETED_TURN_THRESHOLD: i64 = 24;

fn db_error(error: sqlx::Error) -> ApiError {
    ApiError::Internal(error.to_string())
}

pub async fn get_or_create_primary_conversation_id_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &str,
    bot_id: &str,
) -> Result<String, ApiError> {
    if let Some(id) = sqlx::query_scalar::<_, String>(
        "SELECT id FROM conversations WHERE bot_id = $1 AND owner_id = $2 ORDER BY updated_at DESC LIMIT 1",
    )
    .bind(bot_id)
    .bind(owner)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    {
        return Ok(id);
    }
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO conversations (id, owner_id, bot_id) VALUES ($1, $2, $3)")
        .bind(&id)
        .bind(owner)
        .bind(bot_id)
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
    Ok(id)
}

pub async fn get_codex_thread_id(
    pool: &PgPool,
    conversation_id: &str,
) -> Result<Option<String>, sqlx::Error> {
    let value: Option<Option<String>> =
        sqlx::query_scalar("SELECT codex_thread_id FROM conversations WHERE id = $1")
            .bind(conversation_id)
            .fetch_optional(pool)
            .await?;
    Ok(value.and_then(|inner| inner))
}

pub async fn set_codex_thread_id(
    pool: &PgPool,
    conversation_id: &str,
    thread_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE conversations SET codex_thread_id = $2, updated_at = NOW() WHERE id = $1",
    )
    .bind(conversation_id)
    .bind(thread_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn clear_codex_thread_id(pool: &PgPool, conversation_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE conversations SET codex_thread_id = NULL, updated_at = NOW() WHERE id = $1",
    )
    .bind(conversation_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn count_completed_assistant_turns(
    pool: &PgPool,
    conversation_id: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages WHERE conversation_id = $1 AND role = 'assistant' AND status = 'complete'",
    )
    .bind(conversation_id)
    .fetch_one(pool)
    .await
}

#[derive(Debug, Clone)]
struct HistoryRow {
    role: String,
    body: String,
}

/// Prior turns for the Responses engine, oldest first, bounded by count and total bytes.
pub async fn load_bounded_responses_history(
    pool: &PgPool,
    conversation_id: &str,
    exclude_message_id: &str,
) -> Result<Vec<Value>, String> {
    let rows = sqlx::query(
        r#"
        SELECT role, body
        FROM messages
        WHERE conversation_id = $1
          AND sequence < COALESCE(
            (SELECT sequence FROM messages WHERE id = $2),
            9223372036854775807
          )
          AND role IN ('user', 'assistant')
          AND status IN ('complete', 'cancelled', 'interrupted', 'error')
        ORDER BY sequence ASC
        "#,
    )
    .bind(conversation_id)
    .bind(exclude_message_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let parsed: Vec<HistoryRow> = rows
        .into_iter()
        .map(|row| HistoryRow {
            role: row.get("role"),
            body: row.get("body"),
        })
        .filter(|row| !row.body.trim().is_empty())
        .collect();

    let mut selected: Vec<HistoryRow> = Vec::new();
    let mut total_bytes = 0usize;
    for row in parsed.into_iter().rev() {
        if selected.len() >= MAX_RESPONSES_HISTORY_MESSAGES {
            break;
        }
        let bytes = row.body.len();
        if total_bytes + bytes > 200_000 && !selected.is_empty() {
            break;
        }
        selected.push(row);
        total_bytes += bytes;
    }
    selected.reverse();

    Ok(selected
        .into_iter()
        .map(|row| {
            json!({
                "role": row.role,
                "content": row.body,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_cap_constant_is_reasonable() {
        assert!(MAX_RESPONSES_HISTORY_MESSAGES >= 20);
        assert!(CODEX_COMPACT_COMPLETED_TURN_THRESHOLD >= 10);
    }
}
