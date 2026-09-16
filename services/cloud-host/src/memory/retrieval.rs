use sqlx::{FromRow, PgPool, Postgres, Transaction};

use super::db::MemoryRecord;
use crate::error::ApiError;

pub const MAX_RUN_MEMORY_ITEMS: usize = 15;
pub const MAX_RUN_MEMORY_BYTES: usize = 8192;

#[derive(Debug, Clone)]
pub struct RetrievedMemory {
    pub record: MemoryRecord,
    pub score: f64,
}

#[derive(Debug, Clone, FromRow)]
struct ScoredRow {
    id: String,
    owner_id: String,
    bot_id: String,
    kind: String,
    content: String,
    search_terms: Vec<String>,
    importance: i16,
    confidence: f32,
    source_kind: String,
    source_run_id: Option<String>,
    source_message_id: Option<String>,
    status: String,
    superseded_by: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    last_confirmed_at: chrono::DateTime<chrono::Utc>,
    last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    use_count: i32,
    score: f64,
}

fn db(error: sqlx::Error) -> ApiError {
    ApiError::Internal(error.to_string())
}

pub fn lexical_query(raw: &str) -> Option<String> {
    let words: Vec<String> = raw
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| word.len() >= 2)
        .take(12)
        .map(|word| word.to_ascii_lowercase())
        .collect();
    if words.is_empty() {
        None
    } else {
        Some(words.join(" "))
    }
}

pub async fn retrieve_relevant_memories(
    pool: &PgPool,
    owner_id: &str,
    bot_id: &str,
    query: &str,
    limit: i64,
) -> Result<Vec<MemoryRecord>, ApiError> {
    Ok(retrieve_scored(pool, owner_id, bot_id, query, limit)
        .await?
        .into_iter()
        .map(|item| item.record)
        .collect())
}

const RETRIEVAL_SQL: &str = r#"
        SELECT
            id, owner_id, bot_id, kind, content, search_terms, importance, confidence,
            source_kind, source_run_id, source_message_id, status, superseded_by,
            created_at, updated_at, last_confirmed_at, last_used_at, use_count,
            (
                COALESCE(ts_rank_cd(search_vector, websearch_to_tsquery('simple', $3)), 0) * 4.0
                + (importance::float / 5.0) * 1.5
                + 1.0 / (1.0 + EXTRACT(EPOCH FROM (NOW() - GREATEST(last_confirmed_at, created_at))) / 2592000.0)
                + LN(1 + GREATEST(use_count, 0)) * 0.15
            ) AS score
        FROM bot_memories
        WHERE owner_id = $1
          AND bot_id = $2
          AND status = 'active'
          AND (
            search_vector @@ websearch_to_tsquery('simple', $3)
            OR content ILIKE '%' || $3 || '%'
            OR importance >= 4
            OR last_confirmed_at > NOW() - INTERVAL '14 days'
          )
        ORDER BY score DESC, last_confirmed_at DESC, id
        LIMIT $4
        "#;

pub async fn retrieve_scored(
    pool: &PgPool,
    owner_id: &str,
    bot_id: &str,
    query: &str,
    limit: i64,
) -> Result<Vec<RetrievedMemory>, ApiError> {
    let Some(search) = lexical_query(query) else {
        return Ok(Vec::new());
    };
    let limit = limit.clamp(1, MAX_RUN_MEMORY_ITEMS as i64);
    let rows: Vec<ScoredRow> = sqlx::query_as(RETRIEVAL_SQL)
        .bind(owner_id)
        .bind(bot_id)
        .bind(&search)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(db)?;
    Ok(select_from_rows(rows, &search))
}

pub async fn retrieve_scored_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    owner_id: &str,
    bot_id: &str,
    query: &str,
    limit: i64,
) -> Result<Vec<RetrievedMemory>, ApiError> {
    let Some(search) = lexical_query(query) else {
        return Ok(Vec::new());
    };
    let limit = limit.clamp(1, MAX_RUN_MEMORY_ITEMS as i64);
    let rows: Vec<ScoredRow> = sqlx::query_as(RETRIEVAL_SQL)
        .bind(owner_id)
        .bind(bot_id)
        .bind(&search)
        .bind(limit)
        .fetch_all(&mut **tx)
        .await
        .map_err(db)?;
    Ok(select_from_rows(rows, &search))
}

fn select_from_rows(rows: Vec<ScoredRow>, query: &str) -> Vec<RetrievedMemory> {
    let mut selected = Vec::new();
    let mut bytes = 0usize;
    for row in rows {
        if row.score < 0.35 && !content_matches(&row.content, query) && row.importance < 4 {
            continue;
        }
        let next_bytes = bytes + row.content.len();
        if selected.len() >= MAX_RUN_MEMORY_ITEMS || next_bytes > MAX_RUN_MEMORY_BYTES {
            break;
        }
        bytes = next_bytes;
        selected.push(RetrievedMemory {
            score: row.score,
            record: MemoryRecord {
                id: row.id,
                owner_id: row.owner_id,
                bot_id: row.bot_id,
                kind: row.kind,
                content: row.content,
                search_terms: row.search_terms,
                importance: row.importance,
                confidence: row.confidence,
                source_kind: row.source_kind,
                source_run_id: row.source_run_id,
                source_message_id: row.source_message_id,
                status: row.status,
                superseded_by: row.superseded_by,
                created_at: row.created_at,
                updated_at: row.updated_at,
                last_confirmed_at: row.last_confirmed_at,
                last_used_at: row.last_used_at,
                use_count: row.use_count,
            },
        });
    }
    selected
}

fn content_matches(content: &str, query: &str) -> bool {
    let content = content.to_ascii_lowercase();
    query
        .split_whitespace()
        .any(|word| word.len() >= 2 && content.contains(word))
}
