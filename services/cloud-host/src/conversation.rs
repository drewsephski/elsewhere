//! Primary conversation resolution and bounded chat history for multi-turn runs.
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::ApiError;

/// Max user+assistant messages passed to the Responses API (excluding the in-flight turn).
pub const MAX_RESPONSES_HISTORY_MESSAGES: usize = 80;

/// Compact Codex threads every N completed assistant turns (24, 48, 72, …).
pub const CODEX_COMPACT_COMPLETED_TURN_INTERVAL: i64 = 24;

/// Returns the milestone turn count when compaction should run, if any.
pub fn codex_compact_milestone_due(
    completed_turns: i64,
    compacted_through_turns: i64,
    interval: i64,
) -> Option<i64> {
    if interval <= 0 || completed_turns < interval {
        return None;
    }
    let milestone = (completed_turns / interval) * interval;
    if milestone > compacted_through_turns {
        Some(milestone)
    } else {
        None
    }
}

fn db_error(error: sqlx::Error) -> ApiError {
    ApiError::Internal(error.to_string())
}

pub async fn get_or_create_primary_conversation_id_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &str,
    bot_id: &str,
) -> Result<String, ApiError> {
    if let Some(id) = sqlx::query_scalar::<_, String>(
        "SELECT id FROM conversations WHERE bot_id = $1 AND owner_id = $2 AND conversation_type = 'direct' ORDER BY updated_at DESC LIMIT 1",
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
    sqlx::query(
        "INSERT INTO conversations (id, owner_id, bot_id, conversation_type) VALUES ($1, $2, $3, 'direct')",
    )
        .bind(&id)
        .bind(owner)
        .bind(bot_id)
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
    Ok(id)
}

pub async fn create_conversation_for_bot(
    pool: &PgPool,
    owner: &str,
    bot_id: &str,
) -> Result<String, ApiError> {
    let bot_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM bots WHERE id = $1 AND owner_id = $2)",
    )
    .bind(bot_id)
    .bind(owner)
    .fetch_one(pool)
    .await
    .map_err(db_error)?;
    if !bot_exists {
        return Err(ApiError::NotFound);
    }
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO conversations (id, owner_id, bot_id, conversation_type) VALUES ($1, $2, $3, 'direct')",
    )
        .bind(&id)
        .bind(owner)
        .bind(bot_id)
        .execute(pool)
        .await
        .map_err(db_error)?;
    Ok(id)
}

pub(crate) async fn ensure_bot_thread_row(
    pool: &PgPool,
    conversation_id: &str,
    bot_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO conversation_bot_threads (conversation_id, bot_id, updated_at)
        VALUES ($1, $2, NOW())
        ON CONFLICT (conversation_id, bot_id) DO NOTHING
        "#,
    )
    .bind(conversation_id)
    .bind(bot_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn ensure_bot_thread_row_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    conversation_id: &str,
    bot_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO conversation_bot_threads (conversation_id, bot_id, updated_at)
        VALUES ($1, $2, NOW())
        ON CONFLICT (conversation_id, bot_id) DO NOTHING
        "#,
    )
    .bind(conversation_id)
    .bind(bot_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn get_codex_thread_id(
    pool: &PgPool,
    conversation_id: &str,
    bot_id: &str,
) -> Result<Option<String>, sqlx::Error> {
    let value: Option<Option<String>> = sqlx::query_scalar(
        "SELECT codex_thread_id FROM conversation_bot_threads WHERE conversation_id = $1 AND bot_id = $2",
    )
    .bind(conversation_id)
    .bind(bot_id)
    .fetch_optional(pool)
    .await?;
    if value.is_some() {
        return Ok(value.and_then(|inner| inner));
    }
    let legacy: Option<Option<String>> =
        sqlx::query_scalar(
            "SELECT codex_thread_id FROM conversations WHERE id = $1 AND bot_id = $2",
        )
        .bind(conversation_id)
        .bind(bot_id)
        .fetch_optional(pool)
        .await?;
    Ok(legacy.and_then(|inner| inner))
}

pub async fn set_codex_thread_id(
    pool: &PgPool,
    conversation_id: &str,
    bot_id: &str,
    thread_id: &str,
) -> Result<(), sqlx::Error> {
    ensure_bot_thread_row(pool, conversation_id, bot_id).await?;
    sqlx::query(
        r#"
        UPDATE conversation_bot_threads
        SET codex_thread_id = $3, updated_at = NOW()
        WHERE conversation_id = $1 AND bot_id = $2
        "#,
    )
    .bind(conversation_id)
    .bind(bot_id)
    .bind(thread_id)
    .execute(pool)
    .await?;
    sqlx::query(
        "UPDATE conversations SET codex_thread_id = $2, updated_at = NOW() WHERE id = $1 AND bot_id = $3",
    )
    .bind(conversation_id)
    .bind(thread_id)
    .bind(bot_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_codex_compacted_through_turns(
    pool: &PgPool,
    conversation_id: &str,
    bot_id: &str,
) -> Result<i64, sqlx::Error> {
    let per_bot: Option<i64> = sqlx::query_scalar(
        "SELECT codex_compacted_through_turns FROM conversation_bot_threads WHERE conversation_id = $1 AND bot_id = $2",
    )
    .bind(conversation_id)
    .bind(bot_id)
    .fetch_optional(pool)
    .await?;
    if let Some(value) = per_bot {
        return Ok(value);
    }
    sqlx::query_scalar(
        "SELECT codex_compacted_through_turns FROM conversations WHERE id = $1 AND bot_id = $2",
    )
    .bind(conversation_id)
    .bind(bot_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| sqlx::Error::RowNotFound)
}

pub async fn set_codex_compacted_through_turns(
    pool: &PgPool,
    conversation_id: &str,
    bot_id: &str,
    turns: i64,
) -> Result<(), sqlx::Error> {
    ensure_bot_thread_row(pool, conversation_id, bot_id).await?;
    sqlx::query(
        r#"
        UPDATE conversation_bot_threads
        SET codex_compacted_through_turns = $3, updated_at = NOW()
        WHERE conversation_id = $1 AND bot_id = $2
        "#,
    )
    .bind(conversation_id)
    .bind(bot_id)
    .bind(turns)
    .execute(pool)
    .await?;
    sqlx::query(
        "UPDATE conversations SET codex_compacted_through_turns = $2, updated_at = NOW() WHERE id = $1 AND bot_id = $3",
    )
    .bind(conversation_id)
    .bind(turns)
    .bind(bot_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn clear_codex_thread_id(
    pool: &PgPool,
    conversation_id: &str,
    bot_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE conversation_bot_threads SET codex_thread_id = NULL, updated_at = NOW() WHERE conversation_id = $1 AND bot_id = $2",
    )
    .bind(conversation_id)
    .bind(bot_id)
    .execute(pool)
    .await?;
    sqlx::query(
        "UPDATE conversations SET codex_thread_id = NULL, updated_at = NOW() WHERE id = $1 AND bot_id = $2",
    )
    .bind(conversation_id)
    .bind(bot_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn count_completed_assistant_turns(
    pool: &PgPool,
    conversation_id: &str,
    bot_id: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM messages
        WHERE conversation_id = $1
          AND author_kind = 'bot'
          AND author_bot_id = $2
          AND status = 'complete'
        "#,
    )
    .bind(conversation_id)
    .bind(bot_id)
    .fetch_one(pool)
    .await
}

#[derive(Debug, Clone)]
struct HistoryRow {
    role: String,
    body: String,
}

pub async fn conversation_type(
    pool: &PgPool,
    conversation_id: &str,
) -> Result<Option<String>, String> {
    sqlx::query_scalar("SELECT conversation_type FROM conversations WHERE id = $1")
        .bind(conversation_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())
}

pub async fn source_message_sequence_for_run(
    pool: &PgPool,
    assistant_message_id: &str,
) -> Result<Option<i64>, String> {
    sqlx::query_scalar(
        r#"
        SELECT m.sequence
        FROM agent_runs r
        JOIN messages m ON m.id = r.source_message_id
        WHERE r.assistant_message_id = $1
        "#,
    )
    .bind(assistant_message_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())
}

/// Input items for a run. Codex uses `thread/resume` for prior context and accepts only the
/// current user turn here; Responses loads bounded Postgres history plus this turn.
pub async fn build_run_input_messages(
    pool: &PgPool,
    conversation_id: &str,
    bot_id: &str,
    assistant_message_id: &str,
    user_message: &str,
    include_prior_turns: bool,
) -> Result<Vec<Value>, String> {
    let user_turn = json!({
        "role": "user",
        "content": user_message,
    });

    let group_context = if conversation_type(pool, conversation_id).await?.as_deref() == Some("group")
    {
        if let Some(source_sequence) =
            source_message_sequence_for_run(pool, assistant_message_id).await?
        {
            crate::group_context::build_group_context_messages(
                pool,
                conversation_id,
                bot_id,
                source_sequence,
            )
            .await?
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    if !include_prior_turns {
        let mut out = group_context;
        out.push(user_turn);
        return Ok(out);
    }
    let mut messages = load_bounded_responses_history(
        pool,
        conversation_id,
        bot_id,
        assistant_message_id,
    )
    .await?;
    messages.extend(group_context);
    messages.push(user_turn);
    Ok(messages)
}

pub async fn commit_group_context_cursor_for_completed_run(
    pool: &PgPool,
    request_id: &str,
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    commit_group_context_cursor_for_completed_run_in_tx(&mut tx, request_id).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn commit_group_context_cursor_for_completed_run_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    request_id: &str,
) -> Result<(), String> {
    let row = sqlx::query(
        r#"
        SELECT r.conversation_id, r.bot_id, r.group_context_through_sequence
        FROM agent_runs r
        WHERE r.request_id = $1 AND r.status = 'completed'
        "#,
    )
    .bind(request_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    let Some(row) = row else {
        return Ok(());
    };
    let conversation_id: String = row.get("conversation_id");
    let conv_type: Option<String> = sqlx::query_scalar(
        "SELECT conversation_type FROM conversations WHERE id = $1",
    )
    .bind(&conversation_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    if conv_type.as_deref() != Some("group") {
        return Ok(());
    }
    let through: Option<i64> = row.get("group_context_through_sequence");
    if let Some(seq) = through {
        crate::group_context::advance_last_seen_group_sequence_in_tx(
            tx,
            &conversation_id,
            row.get("bot_id"),
            seq,
        )
        .await?;
    }
    Ok(())
}

pub async fn record_pending_group_context_boundary(
    pool: &PgPool,
    run_id: &str,
    through_sequence: i64,
) -> Result<(), String> {
    sqlx::query(
        r#"
        UPDATE agent_runs
        SET group_context_through_sequence = $2
        WHERE id = $1 AND group_context_through_sequence IS NULL
        "#,
    )
    .bind(run_id)
    .bind(through_sequence)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn advance_group_context_cursor_for_run(
    pool: &PgPool,
    conversation_id: &str,
    bot_id: &str,
    assistant_message_id: &str,
) -> Result<(), String> {
    if conversation_type(pool, conversation_id).await?.as_deref() != Some("group") {
        return Ok(());
    }
    let sequence = source_message_sequence_for_run(pool, assistant_message_id).await?;
    if let Some(seq) = sequence {
        crate::group_context::advance_last_seen_group_sequence(pool, conversation_id, bot_id, seq)
            .await?;
    }
    Ok(())
}

/// Prior turns for the Responses engine, oldest first, bounded by count and total bytes.
pub async fn load_bounded_responses_history(
    pool: &PgPool,
    conversation_id: &str,
    bot_id: &str,
    exclude_message_id: &str,
) -> Result<Vec<Value>, String> {
    let rows = sqlx::query(
        r#"
        SELECT role, body
        FROM messages
        WHERE conversation_id = $1
          AND sequence < GREATEST(
            COALESCE(
              (SELECT sequence - 1 FROM messages WHERE id = $2),
              0
            ),
            0
          )
          AND status IN ('complete', 'cancelled', 'interrupted', 'error')
          AND (
            (role = 'user' AND author_kind = 'human')
            OR (
              role = 'assistant'
              AND (author_bot_id IS NULL OR author_bot_id = $3)
            )
          )
        ORDER BY sequence ASC
        "#,
    )
    .bind(conversation_id)
    .bind(exclude_message_id)
    .bind(bot_id)
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
    use sqlx::PgPool;
    use uuid::Uuid;

    #[test]
    fn history_cap_constant_is_reasonable() {
        assert!(MAX_RESPONSES_HISTORY_MESSAGES >= 20);
        assert!(CODEX_COMPACT_COMPLETED_TURN_INTERVAL >= 10);
    }

    #[test]
    fn compaction_runs_on_interval_milestones_only() {
        assert_eq!(
            codex_compact_milestone_due(23, 0, 24),
            None
        );
        assert_eq!(
            codex_compact_milestone_due(24, 0, 24),
            Some(24)
        );
        assert_eq!(
            codex_compact_milestone_due(47, 24, 24),
            None
        );
        assert_eq!(
            codex_compact_milestone_due(48, 24, 24),
            Some(48)
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn responses_history_excludes_inflight_user_and_assistant(pool: PgPool) {
        let bot_id = Uuid::new_v4().to_string();
        let conv_id = Uuid::new_v4().to_string();
        let user_prev = Uuid::new_v4().to_string();
        let asst_prev = Uuid::new_v4().to_string();
        let user_current = Uuid::new_v4().to_string();
        let asst_current = Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO bots (id, owner_id, name, system_prompt, model) VALUES ($1, 'alice', 't', '', 'gpt-5.6-luna')",
        )
        .bind(&bot_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO conversations (id, owner_id, bot_id) VALUES ($1, 'alice', $2)",
        )
        .bind(&conv_id)
        .bind(&bot_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO messages (id, conversation_id, role, body, status, sequence) VALUES
             ($1, $3, 'user', 'earlier question', 'complete', 1),
             ($2, $3, 'assistant', 'earlier answer', 'complete', 2),
             ($4, $3, 'user', 'current question', 'complete', 3),
             ($5, $3, 'assistant', '', 'pending', 4)",
        )
        .bind(&user_prev)
        .bind(&asst_prev)
        .bind(&conv_id)
        .bind(&user_current)
        .bind(&asst_current)
        .execute(&pool)
        .await
        .unwrap();

        let history =
            load_bounded_responses_history(&pool, &conv_id, &bot_id, &asst_current)
                .await
                .unwrap();
        let bodies: Vec<String> = history
            .iter()
            .filter_map(|v| v.get("content").and_then(|c| c.as_str()).map(str::to_string))
            .collect();

        assert_eq!(bodies, vec!["earlier question", "earlier answer"]);

        let mut input = history;
        input.push(json!({"role": "user", "content": "current question"}));
        let current_count = input
            .iter()
            .filter(|v| v.get("content") == Some(&json!("current question")))
            .count();
        assert_eq!(current_count, 1);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn responses_history_excludes_system_user_messages(pool: PgPool) {
        let bot_id = Uuid::new_v4().to_string();
        let conv_id = Uuid::new_v4().to_string();
        let system_id = Uuid::new_v4().to_string();
        let asst_current = Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO bots (id, owner_id, name, system_prompt, model) VALUES ($1, 'alice', 't', '', 'gpt-5.6-luna')",
        )
        .bind(&bot_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO conversations (id, owner_id, bot_id) VALUES ($1, 'alice', $2)",
        )
        .bind(&conv_id)
        .bind(&bot_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO messages (id, conversation_id, role, body, status, sequence, author_kind) VALUES
             ($1, $2, 'user', 'Routine started', 'complete', 1, 'system'),
             ($3, $2, 'assistant', '', 'pending', 2, 'bot')",
        )
        .bind(&system_id)
        .bind(&conv_id)
        .bind(&asst_current)
        .execute(&pool)
        .await
        .unwrap();

        let history =
            load_bounded_responses_history(&pool, &conv_id, &bot_id, &asst_current)
                .await
                .unwrap();
        assert!(history.is_empty());
    }
}
