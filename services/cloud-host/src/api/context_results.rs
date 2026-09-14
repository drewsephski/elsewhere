use crate::{
    auth::Principal,
    bot_context::{BotContext, ContextInput},
    error::ApiError,
    AppState,
};
use axum::{
    extract::{Path, State},
    http::{header, HeaderValue},
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::Serialize;
use sqlx::PgPool;

fn db(error: sqlx::Error) -> ApiError {
    ApiError::Internal(error.to_string())
}

pub async fn get_context(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<String>,
) -> Result<Json<BotContext>, ApiError> {
    Ok(Json(
        crate::bot_context::get(&state.pool, owner.owner_id(), &id).await?,
    ))
}
pub async fn save_context(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<String>,
    Json(input): Json<ContextInput>,
) -> Result<Json<BotContext>, ApiError> {
    Ok(Json(
        crate::bot_context::save(&state.pool, owner.owner_id(), &id, input).await?,
    ))
}

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ResultItem {
    pub id: uuid::Uuid,
    pub run_id: String,
    pub name: String,
    pub kind: String,
    pub size: i32,
    pub bot_name: String,
    pub task: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
async fn items(pool: &PgPool, owner: &str, run: Option<&str>) -> Result<Vec<ResultItem>, ApiError> {
    sqlx::query_as("SELECT f.id, f.run_id, f.name, f.kind, octet_length(f.content) AS size, b.name AS bot_name, LEFT(COALESCE(q.user_message, 'Delegated work'), 180) AS task, f.created_at FROM work_results f JOIN agent_runs r ON r.id = f.run_id JOIN bots b ON b.id = r.bot_id LEFT JOIN work_queue q ON q.run_id = r.id WHERE r.owner_id = $1 AND ($2::text IS NULL OR r.id = $2) ORDER BY f.created_at DESC, f.id LIMIT 100")
        .bind(owner).bind(run).fetch_all(pool).await.map_err(db)
}
pub async fn list_results(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
) -> Result<Json<Vec<ResultItem>>, ApiError> {
    Ok(Json(items(&state.pool, owner.owner_id(), None).await?))
}
#[derive(Serialize)]
pub struct RunResults {
    pub items: Vec<ResultItem>,
    pub collecting: bool,
    pub note: Option<String>,
}
pub async fn run_results(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<String>,
) -> Result<Json<RunResults>, ApiError> {
    let (collecting, note): (bool, Option<String>) = sqlx::query_as("SELECT (status IN ('queued','running') OR (started_at IS NOT NULL AND execution_released_at IS NULL)) AS collecting, results_note FROM agent_runs WHERE id = $1 AND owner_id = $2")
        .bind(&id).bind(owner.owner_id()).fetch_optional(&state.pool).await.map_err(db)?.ok_or(ApiError::NotFound)?;
    Ok(Json(RunResults {
        items: items(&state.pool, owner.owner_id(), Some(&id)).await?,
        collecting,
        note,
    }))
}
pub async fn download(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let id = uuid::Uuid::parse_str(&id).map_err(|_| ApiError::NotFound)?;
    let (name, content): (String, Vec<u8>) = sqlx::query_as("SELECT f.name, f.content FROM work_results f JOIN agent_runs r ON r.id = f.run_id WHERE f.id = $1 AND r.owner_id = $2")
        .bind(id).bind(owner.owner_id()).fetch_optional(&state.pool).await.map_err(db)?.ok_or(ApiError::NotFound)?;
    let filename: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._- ".contains(c) {
                c
            } else {
                '_'
            }
        })
        .take(200)
        .collect();
    let disposition = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
        .map_err(|_| ApiError::Internal("Could not prepare download".into()))?;
    let mut response = content.into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    headers.insert(header::CONTENT_DISPOSITION, disposition);
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}
