use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::connectors::secret::ConnectorSecretBox;
use crate::error::ApiError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InstallRow {
    pub id: Uuid,
    pub owner_id: String,
    pub kind: String,
    pub display_name: String,
    pub endpoint_url: String,
    pub config: Value,
    pub enabled: bool,
    pub status: String,
    pub last_discovery_error: Option<String>,
    pub last_discovered_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InstallToolRow {
    pub id: Uuid,
    pub install_id: Uuid,
    pub remote_name: String,
    pub display_name: String,
    pub description: String,
    pub input_schema: Value,
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredSecret {
    pub kind: String,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub header_name: Option<String>,
    #[serde(default)]
    pub access_token: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub token_endpoint: Option<String>,
}

impl StoredSecret {
    pub fn none() -> Self {
        Self {
            kind: "none".into(),
            token: None,
            header_name: None,
            access_token: None,
            refresh_token: None,
            expires_at: None,
            token_type: None,
            client_id: None,
            token_endpoint: None,
        }
    }

    pub fn bearer(token: &str) -> Self {
        Self {
            kind: "bearer".into(),
            token: Some(token.to_string()),
            ..Self::none()
        }
    }

    pub fn api_key_header(header_name: &str, token: &str) -> Self {
        Self {
            kind: "api_key_header".into(),
            header_name: Some(header_name.to_string()),
            token: Some(token.to_string()),
            ..Self::none()
        }
    }

    pub fn secret_values(&self) -> Vec<String> {
        let mut out = Vec::new();
        for value in [
            self.token.as_deref(),
            self.access_token.as_deref(),
            self.refresh_token.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            if !value.is_empty() {
                out.push(value.to_string());
            }
        }
        out
    }
}

pub async fn list_installs(pool: &PgPool, owner_id: &str) -> Result<Vec<InstallRow>, ApiError> {
    sqlx::query_as(
        r#"
        SELECT id, owner_id, kind, display_name, endpoint_url, config, enabled, status,
               last_discovery_error, last_discovered_at, created_at, updated_at
        FROM connector_installs
        WHERE owner_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(owner_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn get_install(
    pool: &PgPool,
    owner_id: &str,
    id: Uuid,
) -> Result<Option<InstallRow>, ApiError> {
    sqlx::query_as(
        r#"
        SELECT id, owner_id, kind, display_name, endpoint_url, config, enabled, status,
               last_discovery_error, last_discovered_at, created_at, updated_at
        FROM connector_installs
        WHERE id = $1 AND owner_id = $2
        "#,
    )
    .bind(id)
    .bind(owner_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn insert_install(
    pool: &PgPool,
    owner_id: &str,
    kind: &str,
    display_name: &str,
    endpoint_url: &str,
    config: &Value,
    status: &str,
    secret: Option<&StoredSecret>,
    secret_box: &ConnectorSecretBox,
    tools: &[InstallToolDraft],
) -> Result<InstallRow, ApiError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let now = Utc::now();
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO connector_installs (
            owner_id, kind, display_name, endpoint_url, config, enabled, status,
            last_discovered_at, created_at, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, TRUE, $6, $7, $7, $7)
        RETURNING id
        "#,
    )
    .bind(owner_id)
    .bind(kind)
    .bind(display_name)
    .bind(endpoint_url)
    .bind(config)
    .bind(status)
    .bind(now)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    if let Some(secret) = secret {
        persist_secret_in_tx(&mut tx, id, secret, secret_box, now).await?;
    }
    replace_tools_in_tx(&mut tx, id, tools).await?;
    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    get_install(pool, owner_id, id)
        .await?
        .ok_or_else(|| ApiError::Internal("install missing after insert".into()))
}

pub async fn update_install_status(
    pool: &PgPool,
    owner_id: &str,
    id: Uuid,
    status: &str,
    enabled: Option<bool>,
    last_error: Option<Option<String>>,
) -> Result<Option<InstallRow>, ApiError> {
    let now = Utc::now();
    let enabled_sql = match enabled {
        Some(value) => Some(value),
        None => None,
    };
    let result = sqlx::query(
        r#"
        UPDATE connector_installs
        SET status = $3,
            enabled = COALESCE($4, enabled),
            last_discovery_error = CASE WHEN $5 THEN $6 ELSE last_discovery_error END,
            updated_at = $7
        WHERE id = $1 AND owner_id = $2
        "#,
    )
    .bind(id)
    .bind(owner_id)
    .bind(status)
    .bind(enabled_sql)
    .bind(last_error.is_some())
    .bind(last_error.clone().flatten())
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    if result.rows_affected() == 0 {
        return Ok(None);
    }
    get_install(pool, owner_id, id).await
}

pub async fn replace_discovered_tools(
    pool: &PgPool,
    owner_id: &str,
    id: Uuid,
    tools: &[InstallToolDraft],
    status: &str,
) -> Result<Option<InstallRow>, ApiError> {
    let existing = get_install(pool, owner_id, id).await?;
    if existing.is_none() {
        return Ok(None);
    }
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    replace_tools_in_tx(&mut tx, id, tools).await?;
    sqlx::query(
        r#"
        UPDATE connector_installs
        SET status = $2,
            last_discovery_error = NULL,
            last_discovered_at = NOW(),
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(status)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    get_install(pool, owner_id, id).await
}

pub async fn delete_install(pool: &PgPool, owner_id: &str, id: Uuid) -> Result<bool, ApiError> {
    let result = sqlx::query("DELETE FROM connector_installs WHERE id = $1 AND owner_id = $2")
        .bind(id)
        .bind(owner_id)
        .execute(pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(result.rows_affected() > 0)
}

pub async fn list_tools_for_owner(
    pool: &PgPool,
    owner_id: &str,
) -> Result<Vec<(InstallRow, InstallToolRow)>, ApiError> {
    let installs = list_installs(pool, owner_id).await?;
    let mut out = Vec::new();
    for install in installs {
        if !install.enabled || install.status != "connected" {
            continue;
        }
        let tools = list_tools(pool, install.id).await?;
        for tool in tools {
            out.push((install.clone(), tool));
        }
    }
    Ok(out)
}

pub async fn list_tools(pool: &PgPool, install_id: Uuid) -> Result<Vec<InstallToolRow>, ApiError> {
    sqlx::query_as(
        r#"
        SELECT id, install_id, remote_name, display_name, description, input_schema, read_only
        FROM connector_install_tools
        WHERE install_id = $1
        ORDER BY remote_name
        "#,
    )
    .bind(install_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn get_tool_for_owner(
    pool: &PgPool,
    owner_id: &str,
    tool_id: Uuid,
) -> Result<Option<(InstallRow, InstallToolRow)>, ApiError> {
    let tool: Option<InstallToolRow> = sqlx::query_as(
        r#"
        SELECT id, install_id, remote_name, display_name, description, input_schema, read_only
        FROM connector_install_tools
        WHERE id = $1
        "#,
    )
    .bind(tool_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    let Some(tool) = tool else {
        return Ok(None);
    };
    let install = get_install(pool, owner_id, tool.install_id).await?;
    Ok(install.map(|row| (row, tool)))
}

pub async fn load_secret(
    pool: &PgPool,
    install_id: Uuid,
    secret_box: &ConnectorSecretBox,
) -> Result<Option<StoredSecret>, ApiError> {
    let row: Option<(Vec<u8>, Vec<u8>)> = sqlx::query_as(
        "SELECT nonce, ciphertext FROM connector_install_secrets WHERE install_id = $1",
    )
    .bind(install_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    let Some((nonce, ciphertext)) = row else {
        return Ok(None);
    };
    let json = secret_box
        .decrypt(&nonce, &ciphertext)
        .map_err(ApiError::Internal)?;
    let secret: StoredSecret =
        serde_json::from_str(&json).map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Some(secret))
}

pub async fn store_secret(
    pool: &PgPool,
    install_id: Uuid,
    secret: &StoredSecret,
    secret_box: &ConnectorSecretBox,
) -> Result<(), ApiError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    persist_secret_in_tx(&mut tx, install_id, secret, secret_box, Utc::now()).await?;
    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct InstallToolDraft {
    pub remote_name: String,
    pub display_name: String,
    pub description: String,
    pub input_schema: Value,
    pub read_only: bool,
}

async fn persist_secret_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    install_id: Uuid,
    secret: &StoredSecret,
    secret_box: &ConnectorSecretBox,
    now: DateTime<Utc>,
) -> Result<(), ApiError> {
    let payload = serde_json::to_string(secret).map_err(|e| ApiError::Internal(e.to_string()))?;
    let (nonce, ciphertext) = secret_box.encrypt(&payload).map_err(ApiError::Internal)?;
    sqlx::query(
        r#"
        INSERT INTO connector_install_secrets (install_id, ciphertext, nonce, updated_at)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (install_id) DO UPDATE
        SET ciphertext = EXCLUDED.ciphertext,
            nonce = EXCLUDED.nonce,
            updated_at = EXCLUDED.updated_at
        "#,
    )
    .bind(install_id)
    .bind(ciphertext)
    .bind(nonce)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(())
}

async fn replace_tools_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    install_id: Uuid,
    tools: &[InstallToolDraft],
) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM connector_install_tools WHERE install_id = $1")
        .bind(install_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    for tool in tools {
        sqlx::query(
            r#"
            INSERT INTO connector_install_tools (
                install_id, remote_name, display_name, description, input_schema, read_only
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(install_id)
        .bind(&tool.remote_name)
        .bind(&tool.display_name)
        .bind(&tool.description)
        .bind(&tool.input_schema)
        .bind(tool.read_only)
        .execute(&mut **tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    }
    Ok(())
}
