//! Owner-bound, short-lived, one-time OAuth state for channel providers.

use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use sqlx::PgPool;

use crate::error::ApiError;

fn db_error(error: sqlx::Error) -> ApiError {
    ApiError::Internal(error.to_string())
}

pub fn random_oauth_state() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub async fn purge_expired(pool: &PgPool) -> Result<u64, ApiError> {
    let result = sqlx::query("DELETE FROM channel_oauth_states WHERE expires_at <= NOW()")
        .execute(pool)
        .await
        .map_err(db_error)?;
    Ok(result.rows_affected())
}

pub async fn store(
    pool: &PgPool,
    state: &str,
    owner_id: &str,
    provider: &str,
    bot_id: &str,
    expires_at: DateTime<Utc>,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        INSERT INTO channel_oauth_states (state, owner_id, provider, bot_id, expires_at, created_at)
        VALUES ($1, $2, $3, $4, $5, NOW())
        "#,
    )
    .bind(state)
    .bind(owner_id)
    .bind(provider)
    .bind(bot_id)
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(db_error)?;
    Ok(())
}

pub struct ConsumedOAuthState {
    pub owner_id: String,
    pub provider: String,
    pub bot_id: Option<String>,
}

pub async fn consume(
    pool: &PgPool,
    state: &str,
    provider: &str,
    owner_id: &str,
) -> Result<Option<ConsumedOAuthState>, ApiError> {
    let row = sqlx::query_as::<_, (String, String, Option<String>)>(
        r#"
        DELETE FROM channel_oauth_states
        WHERE state = $1
          AND provider = $2
          AND owner_id = $3
          AND expires_at > NOW()
        RETURNING owner_id, provider, bot_id
        "#,
    )
    .bind(state)
    .bind(provider)
    .bind(owner_id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    Ok(row.map(|(owner_id, provider, bot_id)| ConsumedOAuthState {
        owner_id,
        provider,
        bot_id,
    }))
}

pub fn default_expiry() -> DateTime<Utc> {
    Utc::now() + Duration::minutes(10)
}
