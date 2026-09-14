//! Explicit, owner-editable context. Context is never an approval or a credential store.
use crate::error::ApiError;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Serialize, sqlx::FromRow)]
pub struct BotContext {
    pub content: String,
    pub revision: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextInput {
    pub content: String,
    pub revision: i64,
}
fn db(error: sqlx::Error) -> ApiError {
    ApiError::Internal(error.to_string())
}

pub async fn get(pool: &PgPool, owner: &str, bot: &str) -> Result<BotContext, ApiError> {
    sqlx::query_as("SELECT COALESCE(c.content, '') AS content, COALESCE(c.revision, 0) AS revision FROM bots b LEFT JOIN bot_context c ON c.bot_id = b.id WHERE b.id = $1 AND b.owner_id = $2")
        .bind(bot).bind(owner).fetch_optional(pool).await.map_err(db)?.ok_or(ApiError::NotFound)
}

pub async fn save(
    pool: &PgPool,
    owner: &str,
    bot: &str,
    input: ContextInput,
) -> Result<BotContext, ApiError> {
    if input.content.len() > 16_000 || input.revision < 0 {
        return Err(ApiError::Validation(
            "Context must be at most 16,000 bytes".into(),
        ));
    }
    let mut tx = pool.begin().await.map_err(db)?;
    let exists: Option<String> =
        sqlx::query_scalar("SELECT id FROM bots WHERE id = $1 AND owner_id = $2 FOR UPDATE")
            .bind(bot)
            .bind(owner)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db)?;
    if exists.is_none() {
        return Err(ApiError::NotFound);
    }
    let revision: Option<i64> =
        sqlx::query_scalar("SELECT revision FROM bot_context WHERE bot_id = $1")
            .bind(bot)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db)?;
    if revision.unwrap_or(0) != input.revision {
        return Err(ApiError::Conflict(
            "Context changed in another window. Reload it before saving.".into(),
        ));
    }
    let context = sqlx::query_as("INSERT INTO bot_context (bot_id, content) VALUES ($1, $2) ON CONFLICT (bot_id) DO UPDATE SET content = EXCLUDED.content, revision = bot_context.revision + 1, updated_at = NOW() RETURNING content, revision")
        .bind(bot).bind(input.content.trim()).fetch_one(&mut *tx).await.map_err(db)?;
    tx.commit().await.map_err(db)?;
    Ok(context)
}
