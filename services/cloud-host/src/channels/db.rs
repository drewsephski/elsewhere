//! Durable channel persistence. Tokens live only in `channel_connection_secrets`.

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::channels::{ORIGIN_CHANNEL, PROVIDER_SLACK};
use crate::connectors::secret::ConnectorSecretBox;
use crate::error::ApiError;

fn db_error(error: sqlx::Error) -> ApiError {
    ApiError::Internal(error.to_string())
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ChannelConnectionRow {
    pub id: String,
    pub owner_id: String,
    pub provider: String,
    pub status: String,
    pub external_workspace_id: Option<String>,
    pub workspace_name: Option<String>,
    pub installer_external_user_id: Option<String>,
    pub bot_user_id: Option<String>,
    pub default_bot_id: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ChannelThreadRow {
    pub id: String,
    pub connection_id: String,
    pub owner_id: String,
    pub bot_id: String,
    pub provider: String,
    pub external_channel_id: String,
    pub external_thread_id: String,
    pub conversation_id: String,
}

pub async fn list_for_owner(
    pool: &PgPool,
    owner_id: &str,
) -> Result<Vec<ChannelConnectionRow>, ApiError> {
    sqlx::query_as(
        r#"
        SELECT id, owner_id, provider, status, external_workspace_id, workspace_name,
               installer_external_user_id, bot_user_id, default_bot_id, enabled,
               created_at, updated_at
        FROM channel_connections
        WHERE owner_id = $1
        ORDER BY updated_at DESC
        "#,
    )
    .bind(owner_id)
    .fetch_all(pool)
    .await
    .map_err(db_error)
}

pub async fn get_for_owner(
    pool: &PgPool,
    owner_id: &str,
    connection_id: &str,
) -> Result<Option<ChannelConnectionRow>, ApiError> {
    sqlx::query_as(
        r#"
        SELECT id, owner_id, provider, status, external_workspace_id, workspace_name,
               installer_external_user_id, bot_user_id, default_bot_id, enabled,
               created_at, updated_at
        FROM channel_connections
        WHERE id = $1 AND owner_id = $2
        "#,
    )
    .bind(connection_id)
    .bind(owner_id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)
}

pub async fn find_connected_slack_workspace(
    pool: &PgPool,
    workspace_id: &str,
) -> Result<Option<ChannelConnectionRow>, ApiError> {
    sqlx::query_as(
        r#"
        SELECT id, owner_id, provider, status, external_workspace_id, workspace_name,
               installer_external_user_id, bot_user_id, default_bot_id, enabled,
               created_at, updated_at
        FROM channel_connections
        WHERE provider = $1 AND external_workspace_id = $2 AND status = 'connected' AND enabled
        LIMIT 1
        "#,
    )
    .bind(PROVIDER_SLACK)
    .bind(workspace_id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)
}

pub async fn upsert_slack_connection(
    pool: &PgPool,
    owner_id: &str,
    workspace_id: &str,
    workspace_name: Option<&str>,
    installer_user_id: &str,
    bot_user_id: Option<&str>,
    default_bot_id: &str,
    access_token: &str,
    secret_box: &ConnectorSecretBox,
) -> Result<ChannelConnectionRow, ApiError> {
    let (nonce, ciphertext) = secret_box
        .encrypt(access_token)
        .map_err(|e| ApiError::Internal(e))?;
    let mut tx = pool.begin().await.map_err(db_error)?;
    sqlx::query(
        r#"
        UPDATE channel_connections
        SET status = 'disconnected', enabled = FALSE, updated_at = NOW()
        WHERE owner_id = $1 AND provider = $2 AND status = 'connected'
          AND COALESCE(external_workspace_id, '') <> $3
        "#,
    )
    .bind(owner_id)
    .bind(PROVIDER_SLACK)
    .bind(workspace_id)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;

    let existing: Option<String> = sqlx::query_scalar(
        r#"
        SELECT id FROM channel_connections
        WHERE owner_id = $1 AND provider = $2 AND external_workspace_id = $3
        FOR UPDATE
        "#,
    )
    .bind(owner_id)
    .bind(PROVIDER_SLACK)
    .bind(workspace_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(db_error)?;

    let id = existing.unwrap_or_else(|| Uuid::new_v4().to_string());
    sqlx::query(
        r#"
        INSERT INTO channel_connections (
            id, owner_id, provider, status, external_workspace_id, workspace_name,
            installer_external_user_id, bot_user_id, default_bot_id, enabled, created_at, updated_at
        ) VALUES ($1,$2,$3,'connected',$4,$5,$6,$7,$8,TRUE,NOW(),NOW())
        ON CONFLICT (id) DO UPDATE SET
            status = 'connected',
            workspace_name = EXCLUDED.workspace_name,
            installer_external_user_id = EXCLUDED.installer_external_user_id,
            bot_user_id = EXCLUDED.bot_user_id,
            default_bot_id = EXCLUDED.default_bot_id,
            enabled = TRUE,
            updated_at = NOW()
        "#,
    )
    .bind(&id)
    .bind(owner_id)
    .bind(PROVIDER_SLACK)
    .bind(workspace_id)
    .bind(workspace_name)
    .bind(installer_user_id)
    .bind(bot_user_id)
    .bind(default_bot_id)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;

    sqlx::query(
        r#"
        INSERT INTO channel_connection_secrets (connection_id, ciphertext, nonce, key_version, updated_at)
        VALUES ($1,$2,$3,1,NOW())
        ON CONFLICT (connection_id) DO UPDATE SET
            ciphertext = EXCLUDED.ciphertext,
            nonce = EXCLUDED.nonce,
            key_version = 1,
            updated_at = NOW()
        "#,
    )
    .bind(&id)
    .bind(&ciphertext)
    .bind(&nonce)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;

    tx.commit().await.map_err(db_error)?;
    get_for_owner(pool, owner_id, &id)
        .await?
        .ok_or_else(|| ApiError::Internal("channel connection missing after upsert".into()))
}

pub async fn set_default_bot(
    pool: &PgPool,
    owner_id: &str,
    connection_id: &str,
    bot_id: &str,
) -> Result<Option<ChannelConnectionRow>, ApiError> {
    let updated = sqlx::query(
        r#"
        UPDATE channel_connections
        SET default_bot_id = $3, updated_at = NOW()
        WHERE id = $1 AND owner_id = $2 AND status = 'connected'
        "#,
    )
    .bind(connection_id)
    .bind(owner_id)
    .bind(bot_id)
    .execute(pool)
    .await
    .map_err(db_error)?;
    if updated.rows_affected() == 0 {
        return Ok(None);
    }
    get_for_owner(pool, owner_id, connection_id).await
}

pub async fn load_access_token(
    pool: &PgPool,
    connection_id: &str,
    secret_box: &ConnectorSecretBox,
) -> Result<Option<String>, ApiError> {
    let row = sqlx::query(
        "SELECT nonce, ciphertext FROM channel_connection_secrets WHERE connection_id = $1",
    )
    .bind(connection_id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let nonce: Vec<u8> = row.get("nonce");
    let ciphertext: Vec<u8> = row.get("ciphertext");
    let token = secret_box
        .decrypt(&nonce, &ciphertext)
        .map_err(|_| ApiError::Internal("could not decrypt channel secret".into()))?;
    Ok(Some(token))
}

pub async fn disconnect(
    pool: &PgPool,
    owner_id: &str,
    connection_id: &str,
) -> Result<bool, ApiError> {
    let mut tx = pool.begin().await.map_err(db_error)?;
    let exists: Option<String> = sqlx::query_scalar(
        "SELECT id FROM channel_connections WHERE id = $1 AND owner_id = $2 FOR UPDATE",
    )
    .bind(connection_id)
    .bind(owner_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(db_error)?;
    let Some(_) = exists else {
        return Ok(false);
    };
    sqlx::query("DELETE FROM channel_connection_secrets WHERE connection_id = $1")
        .bind(connection_id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    sqlx::query(
        r#"
        UPDATE channel_connections
        SET status = 'disconnected', enabled = FALSE, updated_at = NOW()
        WHERE id = $1 AND owner_id = $2
        "#,
    )
    .bind(connection_id)
    .bind(owner_id)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    Ok(true)
}

pub async fn insert_received_event(
    pool: &PgPool,
    provider: &str,
    external_event_id: &str,
    idempotency_key: &str,
    event_type: Option<&str>,
    connection_id: Option<&str>,
    owner_id: Option<&str>,
    payload: &serde_json::Value,
) -> Result<Option<String>, ApiError> {
    let id = Uuid::new_v4().to_string();
    let inserted = sqlx::query_scalar::<_, String>(
        r#"
        INSERT INTO channel_events (
            id, connection_id, owner_id, provider, external_event_id, idempotency_key,
            event_type, status, payload_json, created_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,'received',$8,NOW())
        ON CONFLICT (provider, external_event_id) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(&id)
    .bind(connection_id)
    .bind(owner_id)
    .bind(provider)
    .bind(external_event_id)
    .bind(idempotency_key)
    .bind(event_type)
    .bind(payload)
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    Ok(inserted)
}

pub async fn list_received_events(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<(String, serde_json::Value)>, ApiError> {
    sqlx::query_as(
        r#"
        SELECT id, COALESCE(payload_json, '{}'::jsonb)
        FROM channel_events
        WHERE status = 'received'
        ORDER BY created_at ASC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(db_error)
}

pub async fn mark_event_status(
    pool: &PgPool,
    event_row_id: &str,
    status: &str,
    ignore_reason: Option<&str>,
    run_id: Option<&str>,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        UPDATE channel_events
        SET status = $2, ignore_reason = $3, run_id = $4
        WHERE id = $1
        "#,
    )
    .bind(event_row_id)
    .bind(status)
    .bind(ignore_reason)
    .bind(run_id)
    .execute(pool)
    .await
    .map_err(db_error)?;
    Ok(())
}

pub async fn events_in_window(
    pool: &PgPool,
    connection_id: &str,
    window_secs: i64,
) -> Result<i64, ApiError> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM channel_events
        WHERE connection_id = $1
          AND created_at > NOW() - ($2 * INTERVAL '1 second')
        "#,
    )
    .bind(connection_id)
    .bind(window_secs)
    .fetch_one(pool)
    .await
    .map_err(db_error)
}

pub async fn get_or_create_thread(
    tx: &mut Transaction<'_, Postgres>,
    connection: &ChannelConnectionRow,
    bot_id: &str,
    channel_id: &str,
    thread_id: &str,
) -> Result<ChannelThreadRow, ApiError> {
    if let Some(existing) = sqlx::query_as::<_, ChannelThreadRow>(
        r#"
        SELECT id, connection_id, owner_id, bot_id, provider, external_channel_id,
               external_thread_id, conversation_id
        FROM channel_threads
        WHERE connection_id = $1 AND external_channel_id = $2 AND external_thread_id = $3
        FOR UPDATE
        "#,
    )
    .bind(&connection.id)
    .bind(channel_id)
    .bind(thread_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    {
        if existing.bot_id != bot_id {
            let conversation_id =
                insert_channel_conversation(tx, &connection.owner_id, bot_id).await?;
            sqlx::query(
                r#"
                UPDATE channel_threads
                SET bot_id = $2, conversation_id = $3, last_activity_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(&existing.id)
            .bind(bot_id)
            .bind(&conversation_id)
            .execute(&mut **tx)
            .await
            .map_err(db_error)?;
            return Ok(ChannelThreadRow {
                bot_id: bot_id.to_string(),
                conversation_id,
                ..existing
            });
        }
        sqlx::query("UPDATE channel_threads SET last_activity_at = NOW() WHERE id = $1")
            .bind(&existing.id)
            .execute(&mut **tx)
            .await
            .map_err(db_error)?;
        return Ok(existing);
    }

    let conversation_id = insert_channel_conversation(tx, &connection.owner_id, bot_id).await?;
    let id = Uuid::new_v4().to_string();
    let inserted = sqlx::query_as::<_, ChannelThreadRow>(
        r#"
        INSERT INTO channel_threads (
            id, connection_id, owner_id, bot_id, provider, external_channel_id,
            external_thread_id, conversation_id, created_at, last_activity_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,NOW(),NOW())
        ON CONFLICT (connection_id, external_channel_id, external_thread_id) DO UPDATE
            SET last_activity_at = NOW()
        RETURNING id, connection_id, owner_id, bot_id, provider, external_channel_id,
                  external_thread_id, conversation_id
        "#,
    )
    .bind(&id)
    .bind(&connection.id)
    .bind(&connection.owner_id)
    .bind(bot_id)
    .bind(&connection.provider)
    .bind(channel_id)
    .bind(thread_id)
    .bind(&conversation_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(db_error)?;
    if inserted.id != id {
        sqlx::query("DELETE FROM conversations WHERE id = $1")
            .bind(&conversation_id)
            .execute(&mut **tx)
            .await
            .map_err(db_error)?;
    }
    Ok(inserted)
}

async fn insert_channel_conversation(
    tx: &mut Transaction<'_, Postgres>,
    owner_id: &str,
    bot_id: &str,
) -> Result<String, ApiError> {
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO conversations (id, owner_id, bot_id, conversation_type, origin_kind)
        VALUES ($1, $2, $3, 'direct', $4)
        "#,
    )
    .bind(&id)
    .bind(owner_id)
    .bind(bot_id)
    .bind(ORIGIN_CHANNEL)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    Ok(id)
}

pub async fn thread_for_conversation(
    pool: &PgPool,
    conversation_id: &str,
) -> Result<Option<ChannelThreadRow>, ApiError> {
    sqlx::query_as(
        r#"
        SELECT id, connection_id, owner_id, bot_id, provider, external_channel_id,
               external_thread_id, conversation_id
        FROM channel_threads
        WHERE conversation_id = $1
        LIMIT 1
        "#,
    )
    .bind(conversation_id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)
}

pub async fn thread_by_id(
    pool: &PgPool,
    thread_id: &str,
) -> Result<Option<ChannelThreadRow>, ApiError> {
    sqlx::query_as(
        r#"
        SELECT id, connection_id, owner_id, bot_id, provider, external_channel_id,
               external_thread_id, conversation_id
        FROM channel_threads WHERE id = $1
        "#,
    )
    .bind(thread_id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)
}
