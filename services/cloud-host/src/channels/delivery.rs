//! Durable outbound channel outbox.
//!
//! Delivery is **at-least-once**. Slack `chat.postMessage` has no idempotency key
//! we can rely on in v1. If the host crashes after Slack accepts a message but
//! before this row is marked `sent`, a later tick may post again.

use chrono::{Duration, Utc};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::bounded_text::truncate_utf8_bytes;
use crate::channels::db::{load_access_token, thread_by_id, thread_for_conversation};
use crate::channels::types::{
    DeliveryKind, MessagingProvider, OutboundDelivery, ProviderDeliveryError,
};
use crate::channels::{
    origin_label, work_url, MAX_DELIVERY_BODY_CHARS, MAX_DELIVERY_CHUNKS, ORIGIN_CHANNEL,
    PROVIDER_SLACK,
};
use crate::connectors::secret::ConnectorSecretBox;
use crate::error::ApiError;

fn db_error(error: sqlx::Error) -> ApiError {
    ApiError::Internal(error.to_string())
}

pub fn split_outbound_body(text: &str, work_url: Option<&str>) -> Vec<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if trimmed.chars().count() <= MAX_DELIVERY_BODY_CHARS {
        return vec![trimmed.to_string()];
    }
    let mut chunks = Vec::new();
    let mut remaining = trimmed;
    while !remaining.is_empty() && chunks.len() < MAX_DELIVERY_CHUNKS {
        if remaining.chars().count() <= MAX_DELIVERY_BODY_CHARS {
            chunks.push(remaining.to_string());
            remaining = "";
            break;
        }
        let mut take = MAX_DELIVERY_BODY_CHARS;
        let prefix: String = remaining.chars().take(take).collect();
        if let Some(idx) = prefix.rfind('\n') {
            if idx > MAX_DELIVERY_BODY_CHARS / 4 {
                take = prefix[..idx].chars().count();
            }
        }
        let chunk: String = remaining.chars().take(take).collect();
        let used = chunk.len();
        chunks.push(chunk);
        remaining = remaining[used..].trim_start();
    }
    if !remaining.is_empty() {
        let link = work_url
            .map(|url| format!("\nContinue in Elsewhere: {url}"))
            .unwrap_or_default();
        if let Some(last) = chunks.last_mut() {
            let budget = MAX_DELIVERY_BODY_CHARS.saturating_sub(link.chars().count());
            *last = format!("{}{link}", truncate_chars(last, budget));
        }
    }
    chunks
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

pub async fn enqueue_assistant_reply_for_run(
    tx: &mut Transaction<'_, Postgres>,
    run_id: &str,
) -> Result<usize, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT r.id, r.owner_id, r.conversation_id, r.origin_kind, r.assistant_message_id
        FROM agent_runs r
        WHERE r.id = $1
        "#,
    )
    .bind(run_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?;
    let Some(row) = row else {
        return Ok(0);
    };
    let origin_kind: String = row.get("origin_kind");
    if origin_kind != ORIGIN_CHANNEL {
        return Ok(0);
    }
    let conversation_id: String = row.get("conversation_id");
    let owner_id: String = row.get("owner_id");
    let assistant_message_id: Option<String> = row.get("assistant_message_id");
    let body: String = if let Some(message_id) = assistant_message_id.as_deref() {
        sqlx::query_scalar("SELECT body FROM messages WHERE id = $1")
            .bind(message_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(db_error)?
            .unwrap_or_default()
    } else {
        String::new()
    };
    if body.trim().is_empty() {
        return Ok(0);
    }
    let thread = sqlx::query_as::<_, super::db::ChannelThreadRow>(
        r#"
        SELECT id, connection_id, owner_id, bot_id, provider, external_channel_id,
               external_thread_id, conversation_id
        FROM channel_threads
        WHERE conversation_id = $1
        LIMIT 1
        "#,
    )
    .bind(&conversation_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?;
    let Some(thread) = thread else {
        return Ok(0);
    };
    let origin = std::env::var("ELSEWHERE_WEB_ORIGIN")
        .ok()
        .filter(|v| !v.trim().is_empty());
    let link = work_url(origin.as_deref(), run_id);
    let chunks = split_outbound_body(&body, link.as_deref());
    insert_chunks(
        tx,
        &thread.connection_id,
        Some(&thread.id),
        &owner_id,
        Some(run_id),
        assistant_message_id.as_deref(),
        DeliveryKind::AssistantReply,
        &chunks,
    )
    .await
}

pub async fn enqueue_assistant_reply_for_run_pool(
    pool: &PgPool,
    run_id: &str,
    web_origin: Option<&str>,
) -> Result<usize, ApiError> {
    let mut tx = pool.begin().await.map_err(db_error)?;
    let inserted = enqueue_assistant_reply_for_run(&mut tx, run_id).await?;
    let _ = web_origin;
    tx.commit().await.map_err(db_error)?;
    Ok(inserted)
}

async fn insert_chunks(
    tx: &mut Transaction<'_, Postgres>,
    connection_id: &str,
    thread_id: Option<&str>,
    owner_id: &str,
    run_id: Option<&str>,
    source_message_id: Option<&str>,
    kind: DeliveryKind,
    chunks: &[String],
) -> Result<usize, ApiError> {
    let mut inserted = 0;
    for (index, body) in chunks.iter().enumerate() {
        let bounded = if body.chars().count() > 4000 {
            truncate_utf8_bytes(body, 4000)
        } else {
            body.clone()
        };
        let id = Uuid::new_v4().to_string();
        let result = sqlx::query(
            r#"
            INSERT INTO channel_deliveries (
                id, connection_id, thread_id, owner_id, source_run_id, source_message_id,
                kind, chunk_index, body, status, attempts, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,'queued',0,NOW())
            ON CONFLICT (source_run_id, kind, chunk_index) DO NOTHING
            "#,
        )
        .bind(&id)
        .bind(connection_id)
        .bind(thread_id)
        .bind(owner_id)
        .bind(run_id)
        .bind(source_message_id)
        .bind(kind.as_str())
        .bind(index as i32)
        .bind(&bounded)
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
        inserted += result.rows_affected() as usize;
    }
    Ok(inserted)
}

pub async fn enqueue_owner_attention(
    pool: &PgPool,
    run_id: &str,
    kind_label: &str,
    web_origin: Option<&str>,
) -> Result<bool, ApiError> {
    enqueue_owner_attention_with_detail(pool, run_id, kind_label, None, web_origin).await
}

pub async fn enqueue_owner_attention_with_detail(
    pool: &PgPool,
    run_id: &str,
    kind_label: &str,
    detail: Option<&str>,
    web_origin: Option<&str>,
) -> Result<bool, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT r.id, r.owner_id, r.conversation_id, r.origin_kind, r.origin_provider, b.name AS bot_name
        FROM agent_runs r
        JOIN bots b ON b.id = r.bot_id
        WHERE r.id = $1
        "#,
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    let Some(row) = row else {
        return Ok(false);
    };
    let origin_kind: String = row.get("origin_kind");
    if origin_kind != ORIGIN_CHANNEL {
        return Ok(false);
    }
    let conversation_id: String = row.get("conversation_id");
    let owner_id: String = row.get("owner_id");
    let bot_name: String = row.get("bot_name");
    let origin_provider: Option<String> = row.get("origin_provider");
    let Some(thread) = thread_for_conversation(pool, &conversation_id).await? else {
        return Ok(false);
    };
    let url = work_url(web_origin, run_id);
    let label = origin_label(&origin_kind, origin_provider.as_deref());
    let _ = label;
    let mut body = format!("{bot_name} needs your {kind_label} in Elsewhere.");
    if let Some(detail) = detail.map(str::trim).filter(|v| !v.is_empty()) {
        body.push_str("\n");
        body.push_str(detail);
    }
    if let Some(url) = url {
        body.push_str("\n");
        body.push_str(&url);
    }
    let mut tx = pool.begin().await.map_err(db_error)?;
    let inserted = insert_chunks(
        &mut tx,
        &thread.connection_id,
        Some(&thread.id),
        &owner_id,
        Some(run_id),
        None,
        DeliveryKind::OwnerAttention,
        &[body],
    )
    .await?;
    tx.commit().await.map_err(db_error)?;
    Ok(inserted > 0)
}

struct ClaimedDelivery {
    id: String,
    connection_id: String,
    thread_id: Option<String>,
    body: String,
}

pub async fn tick(state: &AppState) -> Result<usize, ApiError> {
    let Some(secret_box) = state.connector_secret_box() else {
        return Ok(0);
    };
    let mut sent = 0;
    for _ in 0..10 {
        match send_one(state, secret_box.as_ref()).await? {
            true => sent += 1,
            false => break,
        }
    }
    Ok(sent)
}

async fn send_one(state: &AppState, secret_box: &ConnectorSecretBox) -> Result<bool, ApiError> {
    let mut tx = state.pool.begin().await.map_err(db_error)?;
    let row = sqlx::query(
        r#"
        SELECT id, connection_id, thread_id, body
        FROM channel_deliveries
        WHERE status IN ('queued', 'retryable')
          AND (next_attempt_at IS NULL OR next_attempt_at <= NOW())
        ORDER BY created_at ASC
        LIMIT 1
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(db_error)?;
    let Some(row) = row else {
        tx.commit().await.map_err(db_error)?;
        return Ok(false);
    };
    let claimed = ClaimedDelivery {
        id: row.get("id"),
        connection_id: row.get("connection_id"),
        thread_id: row.get("thread_id"),
        body: row.get("body"),
    };
    sqlx::query(
        "UPDATE channel_deliveries SET status = 'sending', attempts = attempts + 1 WHERE id = $1",
    )
    .bind(&claimed.id)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;

    let Some(token) = load_access_token(&state.pool, &claimed.connection_id, secret_box).await?
    else {
        mark_permanent(&state.pool, &claimed.id, "missing_token").await?;
        return Ok(true);
    };
    let thread = match claimed.thread_id.as_deref() {
        Some(id) => thread_by_id(&state.pool, id).await?,
        None => None,
    };
    let Some(thread) = thread else {
        mark_permanent(&state.pool, &claimed.id, "missing_thread").await?;
        return Ok(true);
    };
    let thread_ts = if thread.external_thread_id == thread.external_channel_id {
        None
    } else {
        Some(thread.external_thread_id.clone())
    };
    if thread.provider != PROVIDER_SLACK {
        mark_permanent(&state.pool, &claimed.id, "unsupported_provider").await?;
        return Ok(true);
    }
    let message = OutboundDelivery {
        channel_id: thread.external_channel_id.clone(),
        thread_ts,
        body: claimed.body.clone(),
    };
    match state.slack_client.deliver(&token, &message).await {
        Ok(receipt) => {
            sqlx::query(
                r#"
                UPDATE channel_deliveries
                SET status = 'sent', provider_message_id = $2, last_error = NULL,
                    sent_at = NOW(), next_attempt_at = NULL
                WHERE id = $1
                "#,
            )
            .bind(&claimed.id)
            .bind(&receipt.provider_message_id)
            .execute(&state.pool)
            .await
            .map_err(db_error)?;
        }
        Err(ProviderDeliveryError::Permanent { message }) => {
            mark_permanent(&state.pool, &claimed.id, &message).await?;
        }
        Err(ProviderDeliveryError::Retryable {
            message,
            retry_after_secs,
        }) => {
            let attempts: i32 =
                sqlx::query_scalar("SELECT attempts FROM channel_deliveries WHERE id = $1")
                    .bind(&claimed.id)
                    .fetch_one(&state.pool)
                    .await
                    .map_err(db_error)?;
            if attempts >= 8 {
                mark_permanent(&state.pool, &claimed.id, &message).await?;
            } else {
                let wait = retry_after_secs
                    .map(|s| Duration::seconds(s as i64))
                    .unwrap_or_else(|| backoff_for_attempt(attempts));
                sqlx::query(
                    r#"
                    UPDATE channel_deliveries
                    SET status = 'retryable', last_error = $2, next_attempt_at = $3
                    WHERE id = $1
                    "#,
                )
                .bind(&claimed.id)
                .bind(crate::redact::redact_secrets(&message))
                .bind(Utc::now() + wait)
                .execute(&state.pool)
                .await
                .map_err(db_error)?;
            }
        }
    }
    Ok(true)
}

fn backoff_for_attempt(attempts: i32) -> Duration {
    match attempts {
        1 => Duration::seconds(2),
        2 => Duration::seconds(8),
        3 => Duration::seconds(30),
        4 => Duration::minutes(2),
        _ => Duration::minutes(5),
    }
}

async fn mark_permanent(pool: &PgPool, id: &str, error: &str) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        UPDATE channel_deliveries
        SET status = 'failed', last_error = $2, next_attempt_at = NULL
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(crate::redact::redact_secrets(error))
    .execute(pool)
    .await
    .map_err(db_error)?;
    Ok(())
}

/// Recover deliveries left in `sending` after a host restart so they retry.
/// This is the at-least-once path: Slack may already have accepted the message.
pub async fn recover_sending(pool: &PgPool) -> Result<u64, ApiError> {
    let result = sqlx::query(
        r#"
        UPDATE channel_deliveries
        SET status = 'retryable', next_attempt_at = NOW(),
            last_error = COALESCE(last_error, 'recovered_after_restart')
        WHERE status = 'sending'
        "#,
    )
    .execute(pool)
    .await
    .map_err(db_error)?;
    Ok(result.rows_affected())
}
