use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::ApiError;
use crate::local_mac::{
    credential_hint, generate_node_credential, generate_pairing_secret, generate_user_code,
    hash_node_credential, hash_pairing_secret, hash_user_code, hashes_equal, verification_url,
    DISPLAY_NAME, PAIRING_TTL, PROVIDER,
};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PairingSessionRow {
    pub id: String,
    pub installation_id: String,
    pub device_name: String,
    pub pairing_secret_hash: Vec<u8>,
    pub user_code_hash: Vec<u8>,
    pub owner_id: Option<String>,
    pub approved_at: Option<DateTime<Utc>>,
    pub consumed_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LocalMacNodeRow {
    pub id: String,
    pub owner_id: String,
    pub sandbox_id: String,
    pub installation_id: String,
    pub device_name: String,
    pub credential_hash: Vec<u8>,
    pub credential_hint: String,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub last_connected_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct CreatedPairing {
    pub pairing_id: String,
    pub pairing_secret: String,
    pub user_code: String,
    pub expires_at: DateTime<Utc>,
    pub verification_url: String,
}

#[derive(Debug, Clone)]
pub struct IssuedNodeCredential {
    pub node_id: String,
    pub computer_id: String,
    pub credential: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExchangeFailure {
    UnknownPairing,
    InvalidSecret,
    Pending,
    Expired,
    Consumed,
}

fn db_error(error: sqlx::Error) -> ApiError {
    ApiError::Internal(error.to_string())
}

pub async fn count_pending_for_installation(
    pool: &PgPool,
    installation_id: &str,
) -> Result<i64, ApiError> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::bigint
        FROM local_mac_pairing_sessions
        WHERE installation_id = $1
          AND consumed_at IS NULL
          AND expires_at > NOW()
        "#,
    )
    .bind(installation_id)
    .fetch_one(pool)
    .await
    .map_err(db_error)
}

pub async fn insert_pairing_session(
    pool: &PgPool,
    key: &[u8; 32],
    installation_id: &str,
    device_name: &str,
    web_origin: Option<&str>,
) -> Result<CreatedPairing, ApiError> {
    let pairing_id = Uuid::new_v4().to_string();
    let pairing_secret = generate_pairing_secret();
    let user_code = generate_user_code();
    let now = Utc::now();
    let expires_at = now
        + chrono::Duration::from_std(PAIRING_TTL).map_err(|e| ApiError::Internal(e.to_string()))?;
    sqlx::query(
        r#"
        INSERT INTO local_mac_pairing_sessions (
            id, installation_id, device_name, pairing_secret_hash, user_code_hash,
            owner_id, approved_at, consumed_at, expires_at, created_at
        ) VALUES ($1, $2, $3, $4, $5, NULL, NULL, NULL, $6, $7)
        "#,
    )
    .bind(&pairing_id)
    .bind(installation_id)
    .bind(device_name)
    .bind(hash_pairing_secret(key, &pairing_secret))
    .bind(hash_user_code(key, &user_code))
    .bind(expires_at)
    .bind(now)
    .execute(pool)
    .await
    .map_err(db_error)?;

    Ok(CreatedPairing {
        pairing_id: pairing_id.clone(),
        pairing_secret,
        user_code: user_code.clone(),
        expires_at,
        verification_url: verification_url(web_origin, &pairing_id, &user_code),
    })
}

pub async fn get_pairing_session(
    pool: &PgPool,
    pairing_id: &str,
) -> Result<Option<PairingSessionRow>, ApiError> {
    sqlx::query_as(
        r#"
        SELECT id, installation_id, device_name, pairing_secret_hash, user_code_hash,
               owner_id, approved_at, consumed_at, expires_at, created_at
        FROM local_mac_pairing_sessions
        WHERE id = $1
        "#,
    )
    .bind(pairing_id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)
}

pub async fn approve_pairing(
    pool: &PgPool,
    key: &[u8; 32],
    pairing_id: &str,
    owner_id: &str,
    user_code: &str,
) -> Result<PairingSessionRow, ApiError> {
    let mut tx = pool.begin().await.map_err(db_error)?;
    let row: Option<PairingSessionRow> = sqlx::query_as(
        r#"
        SELECT id, installation_id, device_name, pairing_secret_hash, user_code_hash,
               owner_id, approved_at, consumed_at, expires_at, created_at
        FROM local_mac_pairing_sessions
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(pairing_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(db_error)?;
    let row = row.ok_or(ApiError::NotFound)?;
    if row.consumed_at.is_some() {
        return Err(ApiError::Conflict("pairing already used".into()));
    }
    if row.expires_at <= Utc::now() {
        return Err(ApiError::Validation("pairing expired".into()));
    }
    if !hashes_equal(&row.user_code_hash, &hash_user_code(key, user_code)) {
        return Err(ApiError::Validation(
            "Could not approve this Mac. Check the code and try again.".into(),
        ));
    }
    if let Some(existing_owner) = row.owner_id.as_deref() {
        if existing_owner != owner_id {
            return Err(ApiError::Conflict("pairing already approved".into()));
        }
        tx.commit().await.map_err(db_error)?;
        return Ok(row);
    }

    let approved: PairingSessionRow = sqlx::query_as(
        r#"
        UPDATE local_mac_pairing_sessions
        SET owner_id = $2, approved_at = NOW()
        WHERE id = $1 AND owner_id IS NULL AND consumed_at IS NULL AND expires_at > NOW()
        RETURNING id, installation_id, device_name, pairing_secret_hash, user_code_hash,
                  owner_id, approved_at, consumed_at, expires_at, created_at
        "#,
    )
    .bind(pairing_id)
    .bind(owner_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(db_error)?
    .ok_or_else(|| ApiError::Conflict("pairing already approved".into()))?;

    tx.commit().await.map_err(db_error)?;
    Ok(approved)
}

pub async fn exchange_pairing(
    pool: &PgPool,
    key: &[u8; 32],
    pairing_id: &str,
    pairing_secret: &str,
) -> Result<IssuedNodeCredential, ExchangeFailure> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| ExchangeFailure::UnknownPairing)?;
    let row: Option<PairingSessionRow> = sqlx::query_as(
        r#"
        SELECT id, installation_id, device_name, pairing_secret_hash, user_code_hash,
               owner_id, approved_at, consumed_at, expires_at, created_at
        FROM local_mac_pairing_sessions
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(pairing_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| ExchangeFailure::UnknownPairing)?;
    let row = row.ok_or(ExchangeFailure::UnknownPairing)?;
    if !hashes_equal(
        &row.pairing_secret_hash,
        &hash_pairing_secret(key, pairing_secret),
    ) {
        return Err(ExchangeFailure::InvalidSecret);
    }
    if row.consumed_at.is_some() {
        return Err(ExchangeFailure::Consumed);
    }
    if row.expires_at <= Utc::now() {
        return Err(ExchangeFailure::Expired);
    }
    if row.approved_at.is_none() || row.owner_id.is_none() {
        return Err(ExchangeFailure::Pending);
    }
    let owner_id = row.owner_id.as_deref().ok_or(ExchangeFailure::Pending)?;
    let credential = generate_node_credential();
    let credential_hash = hash_node_credential(key, &credential);
    let hint = credential_hint(&credential);

    let issued = upsert_node_for_installation(
        &mut tx,
        owner_id,
        &row.installation_id,
        &row.device_name,
        &credential_hash,
        &hint,
    )
    .await?;

    let consumed = sqlx::query(
        r#"
        UPDATE local_mac_pairing_sessions
        SET consumed_at = NOW()
        WHERE id = $1 AND consumed_at IS NULL
        "#,
    )
    .bind(pairing_id)
    .execute(&mut *tx)
    .await
    .map_err(|_| ExchangeFailure::UnknownPairing)?;
    if consumed.rows_affected() != 1 {
        return Err(ExchangeFailure::Consumed);
    }

    tx.commit()
        .await
        .map_err(|_| ExchangeFailure::UnknownPairing)?;
    Ok(IssuedNodeCredential {
        node_id: issued.0,
        computer_id: issued.1,
        credential,
        display_name: DISPLAY_NAME.to_string(),
    })
}

async fn upsert_node_for_installation(
    tx: &mut Transaction<'_, Postgres>,
    owner_id: &str,
    installation_id: &str,
    device_name: &str,
    credential_hash: &[u8],
    credential_hint: &str,
) -> Result<(String, String), ExchangeFailure> {
    let existing: Option<(String, String)> = sqlx::query_as(
        r#"
        SELECT id, sandbox_id
        FROM local_mac_nodes
        WHERE owner_id = $1 AND installation_id = $2
        FOR UPDATE
        "#,
    )
    .bind(owner_id)
    .bind(installation_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|_| ExchangeFailure::UnknownPairing)?;

    if let Some((node_id, sandbox_id)) = existing {
        sqlx::query(
            r#"
            UPDATE local_mac_nodes
            SET device_name = $2,
                credential_hash = $3,
                credential_hint = $4,
                revoked_at = NULL
            WHERE id = $1
            "#,
        )
        .bind(&node_id)
        .bind(device_name)
        .bind(credential_hash)
        .bind(credential_hint)
        .execute(&mut **tx)
        .await
        .map_err(|_| ExchangeFailure::UnknownPairing)?;
        sqlx::query(
            r#"
            UPDATE sandboxes
            SET display_name = $3,
                provider = $4,
                provider_resource_id = $5,
                state = 'active',
                updated_at = NOW()
            WHERE id = $1 AND owner_id = $2
            "#,
        )
        .bind(&sandbox_id)
        .bind(owner_id)
        .bind(DISPLAY_NAME)
        .bind(PROVIDER)
        .bind(&node_id)
        .execute(&mut **tx)
        .await
        .map_err(|_| ExchangeFailure::UnknownPairing)?;
        return Ok((node_id, sandbox_id));
    }

    let node_id = Uuid::new_v4().to_string();
    let sandbox_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    sqlx::query(
        r#"
        INSERT INTO sandboxes (
            id, owner_id, display_name, provider, provider_resource_id, state,
            created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, 'active', $6, $6)
        "#,
    )
    .bind(&sandbox_id)
    .bind(owner_id)
    .bind(DISPLAY_NAME)
    .bind(PROVIDER)
    .bind(&node_id)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|_| ExchangeFailure::UnknownPairing)?;
    sqlx::query(
        r#"
        INSERT INTO local_mac_nodes (
            id, owner_id, sandbox_id, installation_id, device_name,
            credential_hash, credential_hint, revoked_at, created_at, last_connected_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, NULL, $8, NULL)
        "#,
    )
    .bind(&node_id)
    .bind(owner_id)
    .bind(&sandbox_id)
    .bind(installation_id)
    .bind(device_name)
    .bind(credential_hash)
    .bind(credential_hint)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|_| ExchangeFailure::UnknownPairing)?;
    Ok((node_id, sandbox_id))
}

pub async fn get_node_for_owner(
    pool: &PgPool,
    owner_id: &str,
    node_id: &str,
) -> Result<Option<LocalMacNodeRow>, ApiError> {
    sqlx::query_as(
        r#"
        SELECT id, owner_id, sandbox_id, installation_id, device_name,
               credential_hash, credential_hint, revoked_at, created_at, last_connected_at
        FROM local_mac_nodes
        WHERE id = $1 AND owner_id = $2
        "#,
    )
    .bind(node_id)
    .bind(owner_id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)
}

pub async fn revoke_node(pool: &PgPool, owner_id: &str, node_id: &str) -> Result<bool, ApiError> {
    let mut tx = pool.begin().await.map_err(db_error)?;
    let node: Option<(String, String, Option<DateTime<Utc>>)> = sqlx::query_as(
        r#"
        SELECT id, sandbox_id, revoked_at
        FROM local_mac_nodes
        WHERE id = $1 AND owner_id = $2
        FOR UPDATE
        "#,
    )
    .bind(node_id)
    .bind(owner_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(db_error)?;
    let Some((_id, sandbox_id, revoked_at)) = node else {
        return Ok(false);
    };
    if revoked_at.is_none() {
        sqlx::query("UPDATE local_mac_nodes SET revoked_at = NOW() WHERE id = $1")
            .bind(node_id)
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
    }
    sqlx::query(
        r#"
        UPDATE sandboxes
        SET state = 'archived', updated_at = NOW()
        WHERE id = $1 AND owner_id = $2 AND state <> 'archived'
        "#,
    )
    .bind(&sandbox_id)
    .bind(owner_id)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    Ok(true)
}

pub async fn validate_node_credential(
    pool: &PgPool,
    key: &[u8; 32],
    node_id: &str,
    credential: &str,
) -> Result<LocalMacNodeRow, ApiError> {
    let node: Option<LocalMacNodeRow> = sqlx::query_as(
        r#"
        SELECT id, owner_id, sandbox_id, installation_id, device_name,
               credential_hash, credential_hint, revoked_at, created_at, last_connected_at
        FROM local_mac_nodes
        WHERE id = $1
        "#,
    )
    .bind(node_id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    let node = node.ok_or(ApiError::Unauthorized)?;
    if node.revoked_at.is_some() {
        return Err(ApiError::Unauthorized);
    }
    if !hashes_equal(
        &node.credential_hash,
        &hash_node_credential(key, credential),
    ) {
        return Err(ApiError::Unauthorized);
    }
    Ok(node)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedLocalMac {
    pub owner_id: String,
    pub node_id: String,
    pub computer_id: String,
    pub installation_id: String,
}

/// True when the hashed credential matches an active, unrevoked local-Mac node.
///
/// Used at WebSocket handshake so unknown/revoked tokens fail with HTTP 401
/// before upgrade. Does not log or return credential material.
pub async fn active_device_credential_exists(
    pool: &PgPool,
    key: &[u8; 32],
    credential: &str,
) -> Result<bool, ApiError> {
    if !credential.starts_with("emac_") {
        return Ok(false);
    }
    let presented_hash = hash_node_credential(key, credential);
    let exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM local_mac_nodes n
            INNER JOIN sandboxes s ON s.id = n.sandbox_id
            WHERE n.credential_hash = $1
              AND n.revoked_at IS NULL
              AND s.provider = $2
              AND s.owner_id = n.owner_id
              AND s.state <> 'archived'
        )
        "#,
    )
    .bind(presented_hash)
    .bind(PROVIDER)
    .fetch_one(pool)
    .await
    .map_err(db_error)?;
    Ok(exists)
}

/// Resolve a presented device credential to the canonical local-Mac identity.
///
/// The credential is hashed and compared without reading stored secret material.
/// Errors never include the credential. Access is only to this node/computer.
pub async fn authenticate_device_session(
    pool: &PgPool,
    key: &[u8; 32],
    credential: &str,
    claimed_node_id: &str,
    claimed_computer_id: &str,
) -> Result<AuthenticatedLocalMac, ApiError> {
    if !credential.starts_with("emac_")
        || claimed_node_id.is_empty()
        || claimed_computer_id.is_empty()
    {
        return Err(ApiError::Unauthorized);
    }
    let presented_hash = hash_node_credential(key, credential);
    let node: Option<LocalMacNodeRow> = sqlx::query_as(
        r#"
        SELECT id, owner_id, sandbox_id, installation_id, device_name,
               credential_hash, credential_hint, revoked_at, created_at, last_connected_at
        FROM local_mac_nodes
        WHERE id = $1
        "#,
    )
    .bind(claimed_node_id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    let Some(node) = node else {
        return Err(ApiError::Unauthorized);
    };
    if node.revoked_at.is_some() {
        return Err(ApiError::Unauthorized);
    }
    if !hashes_equal(&node.credential_hash, &presented_hash) {
        return Err(ApiError::Unauthorized);
    }
    if node.sandbox_id != claimed_computer_id {
        return Err(ApiError::Unauthorized);
    }

    let sandbox: Option<(String, String, String)> =
        sqlx::query_as("SELECT owner_id, provider, state FROM sandboxes WHERE id = $1")
            .bind(&node.sandbox_id)
            .fetch_optional(pool)
            .await
            .map_err(db_error)?;
    let Some((owner_id, provider, state)) = sandbox else {
        return Err(ApiError::Unauthorized);
    };
    if owner_id != node.owner_id || provider != PROVIDER || state == "archived" {
        return Err(ApiError::Unauthorized);
    }

    Ok(AuthenticatedLocalMac {
        owner_id: node.owner_id,
        node_id: node.id,
        computer_id: node.sandbox_id,
        installation_id: node.installation_id,
    })
}

pub async fn touch_node_connected(pool: &PgPool, node_id: &str) -> Result<(), ApiError> {
    sqlx::query("UPDATE local_mac_nodes SET last_connected_at = NOW() WHERE id = $1")
        .bind(node_id)
        .execute(pool)
        .await
        .map_err(db_error)?;
    Ok(())
}

pub async fn count_sandboxes_for_owner(pool: &PgPool, owner_id: &str) -> Result<i64, ApiError> {
    sqlx::query_scalar("SELECT COUNT(*)::bigint FROM sandboxes WHERE owner_id = $1")
        .bind(owner_id)
        .fetch_one(pool)
        .await
        .map_err(db_error)
}
