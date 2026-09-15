use agent_core::{
    filter_workspace_listing, normalize_workspace_path, validate_public_http_url,
    validate_workspace_mutation_path, validate_workspace_readable_path, workspace_rename_target,
    AgentComputer, ComputerError, ToolError,
};
use serde_json::json;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Extension;
use axum::Json;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use crate::app_state::AppState;
use crate::auth::Principal;
use crate::db::resources::{
    archive_computer, get_computer_for_owner, insert_computer_placeholder, list_computers,
};
use crate::error::ApiError;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerResponse {
    pub id: String,
    pub display_name: String,
    pub provider: String,
    pub state: String,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    pub provider_metadata: ProviderMetadata,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderMetadata {
    pub provisioned: bool,
}

fn to_response(row: crate::db::resources::SandboxRow) -> ComputerResponse {
    ComputerResponse {
        id: row.id.clone(),
        display_name: if row.display_name.is_empty() {
            row.id.clone()
        } else {
            row.display_name
        },
        provider: row.provider,
        state: row.state.clone(),
        last_used_at: row.last_used_at,
        provider_metadata: ProviderMetadata {
            provisioned: row.state == "active",
        },
    }
}

pub async fn list(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
) -> Result<Json<Vec<ComputerResponse>>, ApiError> {
    let rows = list_computers(&state.pool, principal.owner_id())
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(rows.into_iter().map(to_response).collect()))
}

#[derive(Debug, Deserialize)]
pub struct CreateComputerRequest {
    #[serde(rename = "displayName")]
    pub display_name: String,
}

pub async fn create(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(body): Json<CreateComputerRequest>,
) -> Result<Json<ComputerResponse>, ApiError> {
    if body.display_name.trim().is_empty() || body.display_name.len() > 100 {
        return Err(ApiError::Validation("Computer names must contain 1 to 100 bytes".into()));
    }
    let row =
        insert_computer_placeholder(&state.pool, principal.owner_id(), body.display_name.trim())
            .await?;
    Ok(Json(to_response(row)))
}

pub async fn get(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(computer_id): Path<String>,
) -> Result<Json<ComputerResponse>, ApiError> {
    let row = get_computer_for_owner(&state.pool, principal.owner_id(), &computer_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(to_response(row)))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserPreviewResponse {
    pub available: bool,
    pub url: Option<String>,
    pub title: Option<String>,
    pub content_type: Option<String>,
    pub image_base64: Option<String>,
    pub captured_at: String,
    pub version: u64,
}

fn preview_etag(version: u64, frame_etag: Option<&str>) -> String {
    if let Some(etag) = frame_etag {
        return format!("\"{etag}\"");
    }
    format!("\"preview-v{version}\"")
}

#[derive(Debug, Deserialize)]
pub struct BrowserNavigateRequest {
    pub url: String,
}

pub async fn browser_navigate(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(computer_id): Path<String>,
    Json(body): Json<BrowserNavigateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let url = body.url.trim();
    if url.is_empty() {
        return Err(ApiError::Validation("url is required".into()));
    }
    validate_public_http_url(url)
        .await
        .map_err(|err| ApiError::Validation(err.message()))?;
    let computer = state
        .computer_registry
        .connect_sprite_computer(
            &state.config,
            &state.pool,
            principal.owner_id(),
            &computer_id,
            state.config.browser_enabled,
        )
        .await?;
    let result = computer
        .browser_invoke("navigate", &json!({ "url": url }))
        .await
        .map_err(map_computer_error)?;
    Ok(Json(result))
}

pub async fn browser_preview(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(computer_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let sprite = state
        .computer_registry
        .connect_sprite(
            &state.config,
            &state.pool,
            principal.owner_id(),
            &computer_id,
            state.config.browser_enabled,
        )
        .await?;

    let cache = sprite
        .read_browser_preview_cache()
        .await
        .map_err(|e| ApiError::Internal(format!("browser preview cache read failed: {e}")))?;

    let etag = preview_etag(cache.version, cache.etag.as_deref());
    if headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|value| value == etag)
    {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }

    let image_base64 = cache
        .image_jpeg
        .as_ref()
        .map(|bytes| BASE64.encode(bytes));

    let body = BrowserPreviewResponse {
        available: cache.available,
        url: cache.url,
        title: cache.title,
        content_type: if cache.available {
            Some(cache.content_type)
        } else {
            None
        },
        image_base64,
        captured_at: cache
            .captured_at
            .unwrap_or_else(|| Utc::now().to_rfc3339()),
        version: cache.version,
    };

    Ok((
        StatusCode::OK,
        [
            (header::ETAG, etag),
            (header::CACHE_CONTROL, "private, no-cache".to_string()),
        ],
        Json(body),
    )
        .into_response())
}

const WORKSPACE_FILE_MAX_BYTES: usize = 1024 * 1024;

#[derive(Debug, Deserialize)]
pub struct WorkspacePathQuery {
    pub path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceListResponse {
    pub path: String,
    pub entries: Vec<agent_core::WorkspaceEntry>,
    pub revision: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRevisionResponse {
    pub revision: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFileResponse {
    pub path: String,
    pub size: usize,
    pub is_binary: bool,
    pub text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WorkspaceRenameRequest {
    pub path: String,
    #[serde(rename = "newName")]
    pub new_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceMutationResponse {
    pub path: String,
    pub revision: u64,
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn map_workspace_validation(err: &'static str) -> ApiError {
    ApiError::Validation(err.into())
}

fn bump_workspace_revision(computer: &dyn AgentComputer) -> u64 {
    computer.record_workspace_mutation("workspace_write", &json!({ "ok": true }));
    computer.workspace_revision()
}

fn map_computer_error(err: ComputerError) -> ApiError {
    match err {
        ComputerError::NotProvisioned => ApiError::Validation(
            "This computer starts when your bot first uses it for work. Send a message to begin.".into(),
        ),
        ComputerError::SandboxRejected(m) | ComputerError::MalformedArguments(m) => {
            ApiError::Validation(m)
        }
        ComputerError::GuestUnavailable(m) => ApiError::Internal(m),
        ComputerError::BootFailed(m) | ComputerError::ExecutionFailed(m) => ApiError::Internal(m),
        ComputerError::Cancelled => ApiError::Internal("cancelled".into()),
    }
}

fn workspace_list_path(query: &WorkspacePathQuery) -> Result<String, ApiError> {
    let path = query.path.as_deref().unwrap_or("/workspace");
    normalize_workspace_path(path).map_err(map_workspace_validation)
}

fn workspace_read_path(query: &WorkspacePathQuery) -> Result<String, ApiError> {
    let path = query.path.as_deref().unwrap_or("/workspace");
    validate_workspace_readable_path(path).map_err(map_workspace_validation)
}

fn sort_workspace_entries(entries: Vec<agent_core::WorkspaceEntry>) -> Vec<agent_core::WorkspaceEntry> {
    let mut entries = filter_workspace_listing(entries);
    entries.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });
    entries
}

fn is_likely_text(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return true;
    }
    let mut control = 0usize;
    for byte in bytes {
        if *byte == 0 {
            return false;
        }
        if *byte < 9 || (*byte > 13 && *byte < 32) {
            control += 1;
        }
    }
    control * 100 / bytes.len() < 2
}

pub async fn workspace_list(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(computer_id): Path<String>,
    Query(query): Query<WorkspacePathQuery>,
) -> Result<Json<WorkspaceListResponse>, ApiError> {
    let path = workspace_list_path(&query)?;
    let computer = state
        .computer_registry
        .connect_sprite_computer(
            &state.config,
            &state.pool,
            principal.owner_id(),
            &computer_id,
            state.config.browser_enabled,
        )
        .await?;

    let revision = computer.workspace_revision();
    let entries = computer
        .list_dir(&path)
        .await
        .map_err(map_computer_error)?;

    Ok(Json(WorkspaceListResponse {
        path,
        entries: sort_workspace_entries(entries),
        revision,
    }))
}

pub async fn workspace_revision(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(computer_id): Path<String>,
) -> Result<Json<WorkspaceRevisionResponse>, ApiError> {
    let computer = state
        .computer_registry
        .connect_sprite_computer(
            &state.config,
            &state.pool,
            principal.owner_id(),
            &computer_id,
            state.config.browser_enabled,
        )
        .await?;

    Ok(Json(WorkspaceRevisionResponse {
        revision: computer.workspace_revision(),
    }))
}

pub async fn workspace_read(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(computer_id): Path<String>,
    Query(query): Query<WorkspacePathQuery>,
) -> Result<Json<WorkspaceFileResponse>, ApiError> {
    let path = workspace_read_path(&query)?;
    if path.ends_with('/') || path == "/workspace" {
        return Err(ApiError::Validation("path must be a file, not a directory".into()));
    }

    let computer = state
        .computer_registry
        .connect_sprite_computer(
            &state.config,
            &state.pool,
            principal.owner_id(),
            &computer_id,
            state.config.browser_enabled,
        )
        .await?;

    let bytes = computer
        .read_file(&path)
        .await
        .map_err(map_computer_error)?;

    if bytes.len() > WORKSPACE_FILE_MAX_BYTES {
        return Err(ApiError::Validation(format!(
            "file exceeds preview limit of {} bytes",
            WORKSPACE_FILE_MAX_BYTES
        )));
    }

    let is_binary = !is_likely_text(&bytes);
    let text = if is_binary {
        None
    } else {
        Some(String::from_utf8_lossy(&bytes).into_owned())
    };

    Ok(Json(WorkspaceFileResponse {
        path,
        size: bytes.len(),
        is_binary,
        text,
    }))
}

pub async fn workspace_delete(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(computer_id): Path<String>,
    Query(query): Query<WorkspacePathQuery>,
) -> Result<Json<WorkspaceMutationResponse>, ApiError> {
    let path = workspace_read_path(&query)?;
    let path = validate_workspace_mutation_path(&path).map_err(map_workspace_validation)?;

    let computer = state
        .computer_registry
        .connect_sprite_computer(
            &state.config,
            &state.pool,
            principal.owner_id(),
            &computer_id,
            state.config.browser_enabled,
        )
        .await?;

    let command = format!("rm -rf -- {}", shell_single_quote(&path));
    let result = computer.exec(&command).await.map_err(map_computer_error)?;
    if !result.ok {
        let message = if result.stderr.trim().is_empty() {
            "Could not delete path".to_string()
        } else {
            result.stderr.trim().to_string()
        };
        return Err(ApiError::Validation(message));
    }

    let revision = bump_workspace_revision(computer.as_ref());
    Ok(Json(WorkspaceMutationResponse { path, revision }))
}

pub async fn workspace_rename(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(computer_id): Path<String>,
    Json(body): Json<WorkspaceRenameRequest>,
) -> Result<Json<WorkspaceMutationResponse>, ApiError> {
    let from = validate_workspace_mutation_path(body.path.trim()).map_err(map_workspace_validation)?;
    let to = workspace_rename_target(&from, &body.new_name).map_err(map_workspace_validation)?;

    let computer = state
        .computer_registry
        .connect_sprite_computer(
            &state.config,
            &state.pool,
            principal.owner_id(),
            &computer_id,
            state.config.browser_enabled,
        )
        .await?;

    if from != to {
        let command = format!(
            "mv -- {} {}",
            shell_single_quote(&from),
            shell_single_quote(&to)
        );
        let result = computer.exec(&command).await.map_err(map_computer_error)?;
        if !result.ok {
            let message = if result.stderr.trim().is_empty() {
                "Could not rename path".to_string()
            } else {
                result.stderr.trim().to_string()
            };
            return Err(ApiError::Validation(message));
        }
    }

    let revision = bump_workspace_revision(computer.as_ref());
    Ok(Json(WorkspaceMutationResponse { path: to, revision }))
}

pub async fn delete(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(computer_id): Path<String>,
) -> Result<axum::http::StatusCode, ApiError> {
    let archived = archive_computer(&state.pool, principal.owner_id(), &computer_id).await?;
    if archived {
        state
            .computer_registry
            .evict(principal.owner_id(), &computer_id);
        Ok(axum::http::StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound)
    }
}
