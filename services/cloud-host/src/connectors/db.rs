use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;

pub const PROVIDER_GITHUB: &str = "github";

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ConnectorRow {
    pub id: Uuid,
    pub owner_id: String,
    pub provider: String,
    pub status: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub connected_at: Option<DateTime<Utc>>,
}

pub async fn list_for_owner(pool: &PgPool, owner_id: &str) -> Result<Vec<ConnectorRow>, ApiError> {
    sqlx::query_as(
        "SELECT id, owner_id, provider, status, metadata, created_at, updated_at, connected_at
         FROM owner_connectors WHERE owner_id = $1 ORDER BY provider",
    )
    .bind(owner_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn get_for_owner(
    pool: &PgPool,
    owner_id: &str,
    provider: &str,
) -> Result<Option<ConnectorRow>, ApiError> {
    sqlx::query_as(
        "SELECT id, owner_id, provider, status, metadata, created_at, updated_at, connected_at
         FROM owner_connectors WHERE owner_id = $1 AND provider = $2",
    )
    .bind(owner_id)
    .bind(provider)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn upsert_connected(
    pool: &PgPool,
    owner_id: &str,
    provider: &str,
    metadata: &Value,
    access_token: &str,
    secret_box: &crate::connectors::secret::ConnectorSecretBox,
) -> Result<ConnectorRow, ApiError> {
    let (nonce, ciphertext) = secret_box
        .encrypt(access_token)
        .map_err(|e| ApiError::Internal(e))?;
    let now = Utc::now();

    let mut tx = pool.begin().await.map_err(|e| ApiError::Internal(e.to_string()))?;

    let connector_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO owner_connectors (owner_id, provider, status, metadata, connected_at, updated_at)
        VALUES ($1, $2, 'connected', $3, $4, $4)
        ON CONFLICT (owner_id, provider) DO UPDATE
        SET status = 'connected',
            metadata = EXCLUDED.metadata,
            connected_at = EXCLUDED.connected_at,
            updated_at = EXCLUDED.updated_at
        RETURNING id
        "#,
    )
    .bind(owner_id)
    .bind(provider)
    .bind(metadata)
    .bind(now)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        r#"
        INSERT INTO owner_connector_secrets (connector_id, ciphertext, nonce, updated_at)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (connector_id) DO UPDATE
        SET ciphertext = EXCLUDED.ciphertext,
            nonce = EXCLUDED.nonce,
            updated_at = EXCLUDED.updated_at
        "#,
    )
    .bind(connector_id)
    .bind(ciphertext)
    .bind(nonce)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    tx.commit().await.map_err(|e| ApiError::Internal(e.to_string()))?;

    get_for_owner(pool, owner_id, provider)
        .await?
        .ok_or(ApiError::Internal("connector missing after upsert".into()))
}

pub async fn disconnect(
    pool: &PgPool,
    owner_id: &str,
    provider: &str,
) -> Result<bool, ApiError> {
    let now = Utc::now();
    let result = sqlx::query(
        r#"
        UPDATE owner_connectors
        SET status = 'disconnected',
            metadata = '{}'::jsonb,
            connected_at = NULL,
            updated_at = $3
        WHERE owner_id = $1 AND provider = $2
        "#,
    )
    .bind(owner_id)
    .bind(provider)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Ok(false);
    }

    sqlx::query(
        r#"
        DELETE FROM owner_connector_secrets
        WHERE connector_id IN (
            SELECT id FROM owner_connectors WHERE owner_id = $1 AND provider = $2
        )
        "#,
    )
    .bind(owner_id)
    .bind(provider)
    .execute(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(true)
}

pub async fn load_access_token(
    pool: &PgPool,
    owner_id: &str,
    provider: &str,
    secret_box: &crate::connectors::secret::ConnectorSecretBox,
) -> Result<Option<String>, ApiError> {
    let row: Option<(Vec<u8>, Vec<u8>, String)> = sqlx::query_as(
        r#"
        SELECT s.nonce, s.ciphertext, c.status
        FROM owner_connectors c
        JOIN owner_connector_secrets s ON s.connector_id = c.id
        WHERE c.owner_id = $1 AND c.provider = $2
        "#,
    )
    .bind(owner_id)
    .bind(provider)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let Some((nonce, ciphertext, status)) = row else {
        return Ok(None);
    };
    if status != "connected" {
        return Ok(None);
    }
    let token = secret_box
        .decrypt(&nonce, &ciphertext)
        .map_err(|e| ApiError::Internal(e))?;
    Ok(Some(token))
}

pub async fn store_oauth_state(
    pool: &PgPool,
    state: &str,
    owner_id: &str,
    provider: &str,
    expires_at: DateTime<Utc>,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO connector_oauth_states (state, owner_id, provider, expires_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(state)
    .bind(owner_id)
    .bind(provider)
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(())
}

pub async fn consume_oauth_state(
    pool: &PgPool,
    state: &str,
    provider: &str,
) -> Result<Option<String>, ApiError> {
    let now = Utc::now();
    let owner_id: Option<String> = sqlx::query_scalar(
        r#"
        DELETE FROM connector_oauth_states
        WHERE state = $1 AND provider = $2 AND expires_at > $3
        RETURNING owner_id
        "#,
    )
    .bind(state)
    .bind(provider)
    .bind(now)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(owner_id)
}

pub async fn purge_expired_oauth_states(pool: &PgPool) -> Result<(), ApiError> {
    let now = Utc::now();
    sqlx::query("DELETE FROM connector_oauth_states WHERE expires_at <= $1")
        .bind(now)
        .execute(pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(())
}
