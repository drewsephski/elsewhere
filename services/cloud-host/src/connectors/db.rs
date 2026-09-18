use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::connectors::github_client::GitHubCredential;
use crate::error::ApiError;

pub const PROVIDER_GITHUB: &str = "github";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHubCredentialLoad {
    Missing,
    Legacy,
    ReconnectRequired,
    App(GitHubCredential),
}

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

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

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

    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    get_for_owner(pool, owner_id, provider)
        .await?
        .ok_or(ApiError::Internal("connector missing after upsert".into()))
}

pub async fn upsert_github_app_credential(
    pool: &PgPool,
    owner_id: &str,
    metadata: &Value,
    credential: &GitHubCredential,
    secret_box: &crate::connectors::secret::ConnectorSecretBox,
) -> Result<ConnectorRow, ApiError> {
    let plaintext = credential.to_plaintext().map_err(ApiError::Internal)?;
    upsert_connected(
        pool,
        owner_id,
        PROVIDER_GITHUB,
        metadata,
        &plaintext,
        secret_box,
    )
    .await
}

pub async fn replace_connector_secret(
    pool: &PgPool,
    owner_id: &str,
    provider: &str,
    plaintext: &str,
    secret_box: &crate::connectors::secret::ConnectorSecretBox,
) -> Result<bool, ApiError> {
    let (nonce, ciphertext) = secret_box.encrypt(plaintext).map_err(ApiError::Internal)?;
    let now = Utc::now();
    let connector_id: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM owner_connectors WHERE owner_id = $1 AND provider = $2")
            .bind(owner_id)
            .bind(provider)
            .fetch_optional(pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    let Some(connector_id) = connector_id else {
        return Ok(false);
    };
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
    .execute(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(true)
}

pub async fn mark_reconnect_required(
    pool: &PgPool,
    owner_id: &str,
    provider: &str,
) -> Result<bool, ApiError> {
    let now = Utc::now();
    let result = sqlx::query(
        r#"
        UPDATE owner_connectors
        SET status = 'reconnect_required',
            updated_at = $3
        WHERE owner_id = $1 AND provider = $2 AND status <> 'disconnected'
        "#,
    )
    .bind(owner_id)
    .bind(provider)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(result.rows_affected() > 0)
}

pub async fn disconnect(pool: &PgPool, owner_id: &str, provider: &str) -> Result<bool, ApiError> {
    let now = Utc::now();
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let connector_id: Option<Uuid> = sqlx::query_scalar(
        r#"
        UPDATE owner_connectors
        SET status = 'disconnected',
            metadata = '{}'::jsonb,
            connected_at = NULL,
            updated_at = $3
        WHERE owner_id = $1 AND provider = $2
        RETURNING id
        "#,
    )
    .bind(owner_id)
    .bind(provider)
    .bind(now)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let Some(connector_id) = connector_id else {
        tx.rollback()
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        return Ok(false);
    };

    sqlx::query("DELETE FROM owner_connector_secrets WHERE connector_id = $1")
        .bind(connector_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    tx.commit()
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

pub async fn load_github_credential(
    pool: &PgPool,
    owner_id: &str,
    secret_box: &crate::connectors::secret::ConnectorSecretBox,
) -> Result<GitHubCredentialLoad, ApiError> {
    let row: Option<(Vec<u8>, Vec<u8>, String)> = sqlx::query_as(
        r#"
        SELECT s.nonce, s.ciphertext, c.status
        FROM owner_connectors c
        JOIN owner_connector_secrets s ON s.connector_id = c.id
        WHERE c.owner_id = $1 AND c.provider = $2
        "#,
    )
    .bind(owner_id)
    .bind(PROVIDER_GITHUB)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let Some((nonce, ciphertext, status)) = row else {
        return Ok(GitHubCredentialLoad::Missing);
    };
    if status == "reconnect_required" {
        return Ok(GitHubCredentialLoad::ReconnectRequired);
    }
    if status != "connected" {
        return Ok(GitHubCredentialLoad::Missing);
    }
    let plaintext = secret_box
        .decrypt(&nonce, &ciphertext)
        .map_err(ApiError::Internal)?;
    match GitHubCredential::from_plaintext(&plaintext) {
        Some(credential) => Ok(GitHubCredentialLoad::App(credential)),
        None => Ok(GitHubCredentialLoad::Legacy),
    }
}

pub async fn decrypt_connector_secret(
    pool: &PgPool,
    owner_id: &str,
    provider: &str,
    secret_box: &crate::connectors::secret::ConnectorSecretBox,
) -> Result<Option<String>, ApiError> {
    let row: Option<(Vec<u8>, Vec<u8>)> = sqlx::query_as(
        r#"
        SELECT s.nonce, s.ciphertext
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
    let Some((nonce, ciphertext)) = row else {
        return Ok(None);
    };
    let plaintext = secret_box
        .decrypt(&nonce, &ciphertext)
        .map_err(ApiError::Internal)?;
    Ok(Some(plaintext))
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
    owner_id: &str,
) -> Result<bool, ApiError> {
    let now = Utc::now();
    let consumed: Option<String> = sqlx::query_scalar(
        r#"
        DELETE FROM connector_oauth_states
        WHERE state = $1 AND provider = $2 AND owner_id = $3 AND expires_at > $4
        RETURNING owner_id
        "#,
    )
    .bind(state)
    .bind(provider)
    .bind(owner_id)
    .bind(now)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(consumed.is_some())
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
