use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use super::secrets::reject_secret_content;
use super::types::{
    clamp_confidence, clamp_importance, normalize_content, MemoryKind, MemorySourceKind,
    MemoryStatus, MAX_ACTIVE_MEMORIES_PER_BOT, MAX_MEMORY_CONTENT_BYTES,
};
use crate::error::ApiError;
use agent_core::{MAX_MEMORY_SEARCH_TERMS, MAX_MEMORY_SEARCH_TERM_BYTES};

fn db(error: sqlx::Error) -> ApiError {
    ApiError::Internal(error.to_string())
}

#[derive(Debug, Clone, FromRow)]
pub struct MemoryRecord {
    pub id: String,
    pub owner_id: String,
    pub bot_id: String,
    pub kind: String,
    pub content: String,
    pub search_terms: Vec<String>,
    pub importance: i16,
    pub confidence: f32,
    pub source_kind: String,
    pub source_run_id: Option<String>,
    pub source_message_id: Option<String>,
    pub status: String,
    pub superseded_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_confirmed_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub use_count: i32,
}

#[derive(Debug, Clone)]
pub struct NewMemory {
    pub owner_id: String,
    pub bot_id: String,
    pub kind: MemoryKind,
    pub content: String,
    pub search_terms: Vec<String>,
    pub importance: i16,
    pub confidence: f32,
    pub source_kind: MemorySourceKind,
    pub source_run_id: Option<String>,
    pub source_message_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryFilters {
    pub query: Option<String>,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone)]
pub struct ActiveMemoryRef {
    pub id: String,
    pub kind: String,
    pub content: String,
}

const SELECT_COLUMNS: &str = r#"
    id, owner_id, bot_id, kind, content, search_terms, importance, confidence,
    source_kind, source_run_id, source_message_id, status, superseded_by,
    created_at, updated_at, last_confirmed_at, last_used_at, use_count
"#;

pub fn sanitize_search_terms(terms: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for term in terms {
        let cleaned = term
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
            .take(MAX_MEMORY_SEARCH_TERM_BYTES)
            .collect::<String>()
            .trim()
            .to_ascii_lowercase();
        if cleaned.is_empty() || out.iter().any(|existing: &String| existing == &cleaned) {
            continue;
        }
        out.push(cleaned);
        if out.len() >= MAX_MEMORY_SEARCH_TERMS {
            break;
        }
    }
    out
}

pub fn validate_content(content: &str) -> Result<String, ApiError> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err(ApiError::Validation("Memory content is required".into()));
    }
    if trimmed.len() > MAX_MEMORY_CONTENT_BYTES {
        return Err(ApiError::Validation(format!(
            "Memory content must be at most {MAX_MEMORY_CONTENT_BYTES} bytes"
        )));
    }
    reject_secret_content(trimmed).map_err(ApiError::Validation)?;
    Ok(trimmed.to_string())
}

pub async fn count_active_memories(
    tx: &mut Transaction<'_, Postgres>,
    owner_id: &str,
    bot_id: &str,
) -> Result<i64, ApiError> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM bot_memories WHERE owner_id = $1 AND bot_id = $2 AND status = 'active'",
    )
    .bind(owner_id)
    .bind(bot_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(db)
}

pub async fn insert_memory(
    tx: &mut Transaction<'_, Postgres>,
    input: NewMemory,
) -> Result<MemoryRecord, ApiError> {
    let content = validate_content(&input.content)?;
    let normalized = normalize_content(&content);
    if normalized.is_empty() {
        return Err(ApiError::Validation("Memory content is required".into()));
    }
    if let Some(existing) =
        find_active_by_normalized(tx, &input.owner_id, &input.bot_id, &normalized).await?
    {
        return mark_reinforced(tx, &input.owner_id, &input.bot_id, &existing.id).await;
    }
    let active = count_active_memories(tx, &input.owner_id, &input.bot_id).await?;
    if active >= MAX_ACTIVE_MEMORIES_PER_BOT {
        return Err(ApiError::Validation(format!(
            "This Bot already has {MAX_ACTIVE_MEMORIES_PER_BOT} active memories"
        )));
    }
    let id = Uuid::new_v4().to_string();
    let terms = sanitize_search_terms(&input.search_terms);
    sqlx::query_as(&format!(
        "INSERT INTO bot_memories (
            id, owner_id, bot_id, kind, content, content_normalized, search_terms,
            importance, confidence, source_kind, source_run_id, source_message_id
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
        RETURNING {SELECT_COLUMNS}"
    ))
    .bind(&id)
    .bind(&input.owner_id)
    .bind(&input.bot_id)
    .bind(input.kind.as_str())
    .bind(&content)
    .bind(&normalized)
    .bind(&terms)
    .bind(clamp_importance(input.importance as i32))
    .bind(clamp_confidence(input.confidence))
    .bind(input.source_kind.as_str())
    .bind(input.source_run_id.as_deref())
    .bind(input.source_message_id.as_deref())
    .fetch_one(&mut **tx)
    .await
    .map_err(db)
}

pub async fn find_active_by_normalized(
    tx: &mut Transaction<'_, Postgres>,
    owner_id: &str,
    bot_id: &str,
    normalized: &str,
) -> Result<Option<MemoryRecord>, ApiError> {
    sqlx::query_as(&format!(
        "SELECT {SELECT_COLUMNS} FROM bot_memories
         WHERE owner_id = $1 AND bot_id = $2 AND status = 'active' AND content_normalized = $3"
    ))
    .bind(owner_id)
    .bind(bot_id)
    .bind(normalized)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db)
}

pub async fn get_memory_for_owner(
    pool: &PgPool,
    owner_id: &str,
    bot_id: &str,
    memory_id: &str,
) -> Result<Option<MemoryRecord>, ApiError> {
    sqlx::query_as(&format!(
        "SELECT {SELECT_COLUMNS} FROM bot_memories
         WHERE id = $1 AND owner_id = $2 AND bot_id = $3"
    ))
    .bind(memory_id)
    .bind(owner_id)
    .bind(bot_id)
    .fetch_optional(pool)
    .await
    .map_err(db)
}

pub async fn list_memories(
    pool: &PgPool,
    owner_id: &str,
    bot_id: &str,
    filters: &MemoryFilters,
) -> Result<Vec<MemoryRecord>, ApiError> {
    let status = filters.status.as_deref().unwrap_or("active");
    MemoryStatus::parse(status).map_err(ApiError::Validation)?;
    let limit = filters.limit.clamp(1, 50);
    let offset = filters.offset.max(0);
    let kind = filters.kind.as_deref();
    if let Some(kind) = kind {
        MemoryKind::parse(kind).map_err(ApiError::Validation)?;
    }
    let query = filters
        .query
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    sqlx::query_as(&format!(
        "SELECT {SELECT_COLUMNS} FROM bot_memories
         WHERE owner_id = $1 AND bot_id = $2 AND status = $3
           AND ($4::text IS NULL OR kind = $4)
           AND (
             $5::text IS NULL
             OR search_vector @@ websearch_to_tsquery('simple', $5)
             OR content ILIKE '%' || $5 || '%'
           )
         ORDER BY last_confirmed_at DESC, created_at DESC
         LIMIT $6 OFFSET $7"
    ))
    .bind(owner_id)
    .bind(bot_id)
    .bind(status)
    .bind(kind)
    .bind(query)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(db)
}

pub async fn list_active_for_extraction(
    tx: &mut Transaction<'_, Postgres>,
    owner_id: &str,
    bot_id: &str,
    limit: i64,
) -> Result<Vec<ActiveMemoryRef>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, kind, content FROM bot_memories
         WHERE owner_id = $1 AND bot_id = $2 AND status = 'active'
         ORDER BY importance DESC, last_confirmed_at DESC
         LIMIT $3",
    )
    .bind(owner_id)
    .bind(bot_id)
    .bind(limit)
    .fetch_all(&mut **tx)
    .await
    .map_err(db)?;
    Ok(rows
        .into_iter()
        .map(|row| ActiveMemoryRef {
            id: row.get("id"),
            kind: row.get("kind"),
            content: row.get("content"),
        })
        .collect())
}

pub async fn update_memory(
    tx: &mut Transaction<'_, Postgres>,
    owner_id: &str,
    bot_id: &str,
    memory_id: &str,
    content: Option<&str>,
    kind: Option<MemoryKind>,
    search_terms: Option<Vec<String>>,
    importance: Option<i16>,
) -> Result<MemoryRecord, ApiError> {
    let current = sqlx::query_as::<_, MemoryRecord>(&format!(
        "SELECT {SELECT_COLUMNS} FROM bot_memories
         WHERE id = $1 AND owner_id = $2 AND bot_id = $3 FOR UPDATE"
    ))
    .bind(memory_id)
    .bind(owner_id)
    .bind(bot_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db)?
    .ok_or(ApiError::NotFound)?;

    let content = if let Some(content) = content {
        validate_content(content)?
    } else {
        current.content.clone()
    };
    let normalized = normalize_content(&content);
    let kind = kind.map(|k| k.as_str().to_string()).unwrap_or(current.kind);
    let terms = search_terms
        .map(|terms| sanitize_search_terms(&terms))
        .unwrap_or(current.search_terms);
    let importance = importance.unwrap_or(current.importance);
    sqlx::query_as(&format!(
        "UPDATE bot_memories SET
            content = $4,
            content_normalized = $5,
            kind = $6,
            search_terms = $7,
            importance = $8,
            last_confirmed_at = NOW(),
            updated_at = NOW()
         WHERE id = $1 AND owner_id = $2 AND bot_id = $3
         RETURNING {SELECT_COLUMNS}"
    ))
    .bind(memory_id)
    .bind(owner_id)
    .bind(bot_id)
    .bind(&content)
    .bind(&normalized)
    .bind(&kind)
    .bind(&terms)
    .bind(clamp_importance(importance as i32))
    .fetch_one(&mut **tx)
    .await
    .map_err(db)
}

pub async fn archive_memory(
    tx: &mut Transaction<'_, Postgres>,
    owner_id: &str,
    bot_id: &str,
    memory_id: &str,
) -> Result<MemoryRecord, ApiError> {
    sqlx::query_as(&format!(
        "UPDATE bot_memories SET status = 'archived', updated_at = NOW()
         WHERE id = $1 AND owner_id = $2 AND bot_id = $3
         RETURNING {SELECT_COLUMNS}"
    ))
    .bind(memory_id)
    .bind(owner_id)
    .bind(bot_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db)?
    .ok_or(ApiError::NotFound)
}

pub async fn mark_reinforced(
    tx: &mut Transaction<'_, Postgres>,
    owner_id: &str,
    bot_id: &str,
    memory_id: &str,
) -> Result<MemoryRecord, ApiError> {
    sqlx::query_as(&format!(
        "UPDATE bot_memories SET last_confirmed_at = NOW(), updated_at = NOW()
         WHERE id = $1 AND owner_id = $2 AND bot_id = $3 AND status = 'active'
         RETURNING {SELECT_COLUMNS}"
    ))
    .bind(memory_id)
    .bind(owner_id)
    .bind(bot_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db)?
    .ok_or(ApiError::NotFound)
}

pub async fn mark_superseded(
    tx: &mut Transaction<'_, Postgres>,
    owner_id: &str,
    bot_id: &str,
    memory_id: &str,
    successor_id: &str,
) -> Result<MemoryRecord, ApiError> {
    sqlx::query_as(&format!(
        "UPDATE bot_memories SET status = 'superseded', superseded_by = $4, updated_at = NOW()
         WHERE id = $1 AND owner_id = $2 AND bot_id = $3 AND status = 'active'
         RETURNING {SELECT_COLUMNS}"
    ))
    .bind(memory_id)
    .bind(owner_id)
    .bind(bot_id)
    .bind(successor_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db)?
    .ok_or(ApiError::NotFound)
}

pub async fn owned_active_ids(
    tx: &mut Transaction<'_, Postgres>,
    owner_id: &str,
    bot_id: &str,
    ids: &[String],
) -> Result<Vec<String>, ApiError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_scalar(
        "SELECT id FROM bot_memories
         WHERE owner_id = $1 AND bot_id = $2 AND status = 'active' AND id = ANY($3)",
    )
    .bind(owner_id)
    .bind(bot_id)
    .bind(ids)
    .fetch_all(&mut **tx)
    .await
    .map_err(db)
}

pub async fn search_memories(
    pool: &PgPool,
    owner_id: &str,
    bot_id: &str,
    query: &str,
    limit: i64,
) -> Result<Vec<MemoryRecord>, ApiError> {
    super::retrieval::retrieve_relevant_memories(pool, owner_id, bot_id, query, limit).await
}

pub async fn touch_used(
    tx: &mut Transaction<'_, Postgres>,
    owner_id: &str,
    bot_id: &str,
    memory_ids: &[String],
) -> Result<(), ApiError> {
    if memory_ids.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "UPDATE bot_memories
         SET last_used_at = NOW(), use_count = use_count + 1, updated_at = NOW()
         WHERE owner_id = $1 AND bot_id = $2 AND id = ANY($3)",
    )
    .bind(owner_id)
    .bind(bot_id)
    .bind(memory_ids)
    .execute(&mut **tx)
    .await
    .map_err(db)?;
    Ok(())
}
