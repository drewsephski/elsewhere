use crate::app_state::AppState;
use crate::auth::Principal;
use crate::computer_control::{self, ControlHolder, HumanControlRequired};
use crate::db::resources::{
    archive_computer, get_computer_for_owner, insert_computer_placeholder, list_computers,
};
use crate::error::ApiError;
use agent_core::{
    filter_workspace_listing, validate_public_http_url, validate_workspace_list_path,
    validate_workspace_mutation_path, validate_workspace_readable_path, workspace_rename_target,
    AgentComputer, ComputerError,
};
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Extension;
use axum::Json;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connected: Option<bool>,
}

fn to_response(row: crate::db::resources::SandboxRow, connected: Option<bool>) -> ComputerResponse {
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
            connected,
        },
    }
}

fn connected_for(
    state: &AppState,
    owner_id: &str,
    row: &crate::db::resources::SandboxRow,
) -> Option<bool> {
    if row.provider == crate::local_mac::PROVIDER {
        Some(state.local_mac_sessions.is_connected(owner_id, &row.id))
    } else {
        None
    }
}

pub async fn list(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
) -> Result<Json<Vec<ComputerResponse>>, ApiError> {
    let rows = list_computers(&state.pool, principal.owner_id())
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(
        rows.into_iter()
            .map(|row| {
                let connected = connected_for(&state, principal.owner_id(), &row);
                to_response(row, connected)
            })
            .collect(),
    ))
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
        return Err(ApiError::Validation(
            "Computer names must contain 1 to 100 bytes".into(),
        ));
    }
    let row =
        insert_computer_placeholder(&state.pool, principal.owner_id(), body.display_name.trim())
            .await?;
    Ok(Json(to_response(row, None)))
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
    let connected = connected_for(&state, principal.owner_id(), &row);
    Ok(Json(to_response(row, connected)))
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
    require_human_control(&state, principal.owner_id(), &computer_id).await?;
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserControlStateResponse {
    pub holder: String,
    pub lease_id: Option<String>,
    pub acquired_at: Option<String>,
    pub heartbeat_at: Option<String>,
    pub you_have_control: bool,
}

fn control_state_response(
    state: computer_control::ComputerControlState,
) -> BrowserControlStateResponse {
    BrowserControlStateResponse {
        holder: match state.holder {
            ControlHolder::Bot => "bot".into(),
            ControlHolder::Human => "human".into(),
        },
        lease_id: state.lease_id,
        acquired_at: state.acquired_at.map(|t| t.to_rfc3339()),
        heartbeat_at: state.heartbeat_at.map(|t| t.to_rfc3339()),
        you_have_control: state.holder == ControlHolder::Human,
    }
}

async fn require_human_control(
    state: &AppState,
    owner_id: &str,
    computer_id: &str,
) -> Result<(), ApiError> {
    get_computer_for_owner(&state.pool, owner_id, computer_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::NotFound)?;
    if !state.config.browser_enabled {
        return Err(ApiError::Validation(
            "browser control is disabled on this host".into(),
        ));
    }
    computer_control::require_active_human_control(&state.pool, owner_id, computer_id)
        .await
        .map_err(map_human_control_error)
}

fn map_human_control_error(err: HumanControlRequired) -> ApiError {
    match err {
        HumanControlRequired::NotHuman => ApiError::Validation(err.message().into()),
        HumanControlRequired::Db(e) => ApiError::Internal(e.to_string()),
    }
}

pub async fn browser_control_state(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(computer_id): Path<String>,
) -> Result<Json<BrowserControlStateResponse>, ApiError> {
    get_computer_for_owner(&state.pool, principal.owner_id(), &computer_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::NotFound)?;
    let snapshot =
        computer_control::get_control_state(&state.pool, principal.owner_id(), &computer_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(control_state_response(snapshot)))
}

pub async fn browser_control_take(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(computer_id): Path<String>,
) -> Result<Json<BrowserControlStateResponse>, ApiError> {
    get_computer_for_owner(&state.pool, principal.owner_id(), &computer_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::NotFound)?;
    if !state.config.browser_enabled {
        return Err(ApiError::Validation(
            "browser control is disabled on this host".into(),
        ));
    }
    let snapshot =
        computer_control::take_human_control(&state.pool, principal.owner_id(), &computer_id)
            .await
            .map_err(|err| match err {
                computer_control::TakeControlError::Db(e) => ApiError::Internal(e.to_string()),
                computer_control::TakeControlError::Conflict(m) => ApiError::Conflict(m),
            })?;
    Ok(Json(control_state_response(snapshot)))
}

pub async fn browser_control_return(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(computer_id): Path<String>,
) -> Result<Json<BrowserControlStateResponse>, ApiError> {
    get_computer_for_owner(&state.pool, principal.owner_id(), &computer_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::NotFound)?;
    let snapshot =
        computer_control::return_control_to_bot(&state.pool, principal.owner_id(), &computer_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    if let Err(err) = state
        .human_interventions
        .resolve_pending_for_computer_handback(principal.owner_id(), &computer_id)
        .await
    {
        tracing::warn!(error = %err, "could not resolve pending human interventions on handback");
    }
    Ok(Json(control_state_response(snapshot)))
}

pub async fn browser_control_heartbeat(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(computer_id): Path<String>,
) -> Result<Json<BrowserControlStateResponse>, ApiError> {
    get_computer_for_owner(&state.pool, principal.owner_id(), &computer_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::NotFound)?;
    if !state.config.browser_enabled {
        return Err(ApiError::Validation(
            "browser control is disabled on this host".into(),
        ));
    }
    let touched =
        computer_control::touch_human_heartbeat(&state.pool, principal.owner_id(), &computer_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    if !touched {
        return Err(ApiError::Validation(
            "No active human browser control lease to heartbeat".into(),
        ));
    }
    let snapshot =
        computer_control::get_control_state(&state.pool, principal.owner_id(), &computer_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(control_state_response(snapshot)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserClickRequest {
    #[serde(default)]
    pub r#ref: Option<String>,
    #[serde(default)]
    pub x_ratio: Option<f64>,
    #[serde(default)]
    pub y_ratio: Option<f64>,
}

pub async fn browser_click(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(computer_id): Path<String>,
    Json(body): Json<BrowserClickRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_human_control(&state, principal.owner_id(), &computer_id).await?;
    let args = if let Some(ref_id) = body.r#ref.filter(|s| !s.is_empty()) {
        json!({ "ref": ref_id })
    } else {
        let x_ratio = body.x_ratio.ok_or_else(|| {
            ApiError::Validation("ref or xRatio/yRatio required for click".into())
        })?;
        let y_ratio = body.y_ratio.ok_or_else(|| {
            ApiError::Validation("ref or xRatio/yRatio required for click".into())
        })?;
        if !(0.0..=1.0).contains(&x_ratio) || !(0.0..=1.0).contains(&y_ratio) {
            return Err(ApiError::Validation(
                "xRatio and yRatio must be between 0 and 1".into(),
            ));
        }
        json!({ "xRatio": x_ratio, "yRatio": y_ratio })
    };
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
    let action = if args.get("ref").is_some() {
        "click"
    } else {
        "click_point"
    };
    let result = computer
        .browser_invoke(action, &args)
        .await
        .map_err(map_computer_error)?;
    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserScrollRequest {
    pub x_ratio: f64,
    pub y_ratio: f64,
    #[serde(default)]
    pub delta_x: f64,
    #[serde(default)]
    pub delta_y: f64,
}

pub async fn browser_scroll(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(computer_id): Path<String>,
    Json(body): Json<BrowserScrollRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_human_control(&state, principal.owner_id(), &computer_id).await?;
    if !(0.0..=1.0).contains(&body.x_ratio) || !(0.0..=1.0).contains(&body.y_ratio) {
        return Err(ApiError::Validation(
            "xRatio and yRatio must be between 0 and 1".into(),
        ));
    }
    if !body.delta_x.is_finite() || !body.delta_y.is_finite() {
        return Err(ApiError::Validation("deltaX and deltaY must be finite".into()));
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
    let result = computer
        .browser_invoke(
            "scroll",
            &json!({
                "xRatio": body.x_ratio,
                "yRatio": body.y_ratio,
                "deltaX": body.delta_x,
                "deltaY": body.delta_y
            }),
        )
        .await
        .map_err(map_computer_error)?;
    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserTypeRequest {
    #[serde(default)]
    pub r#ref: Option<String>,
    pub text: String,
    #[serde(default)]
    pub submit: Option<bool>,
}

pub async fn browser_type(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(computer_id): Path<String>,
    Json(body): Json<BrowserTypeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_human_control(&state, principal.owner_id(), &computer_id).await?;
    if body.text.is_empty() {
        return Err(ApiError::Validation("text is required".into()));
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
    let submit = body.submit.unwrap_or(false);
    let result = if let Some(ref_id) = body.r#ref.filter(|s| !s.is_empty()) {
        computer
            .browser_invoke(
                "type",
                &json!({
                    "ref": ref_id,
                    "text": body.text,
                    "submit": submit
                }),
            )
            .await
            .map_err(map_computer_error)?
    } else {
        computer
            .browser_invoke(
                "type_focused",
                &json!({
                    "text": body.text,
                    "submit": submit
                }),
            )
            .await
            .map_err(map_computer_error)?
    };
    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserPressKeyRequest {
    pub key: String,
}

pub async fn browser_press_key(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(computer_id): Path<String>,
    Json(body): Json<BrowserPressKeyRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_human_control(&state, principal.owner_id(), &computer_id).await?;
    let key = body.key.trim();
    if key.is_empty() {
        return Err(ApiError::Validation("key is required".into()));
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
    let result = computer
        .browser_invoke("press", &json!({ "key": key }))
        .await
        .map_err(map_computer_error)?;
    Ok(Json(result))
}

pub async fn browser_close(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(computer_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_human_control(&state, principal.owner_id(), &computer_id).await?;
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
        .browser_invoke("close", &json!({}))
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

    let image_base64 = cache.image_jpeg.as_ref().map(|bytes| BASE64.encode(bytes));

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
        captured_at: cache.captured_at.unwrap_or_else(|| Utc::now().to_rfc3339()),
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
            "This computer starts when your bot first uses it for work. Send a message to begin."
                .into(),
        ),
        ComputerError::SandboxRejected(m) | ComputerError::MalformedArguments(m) => {
            ApiError::Validation(m)
        }
        ComputerError::GuestUnavailable(m) => ApiError::Internal(m),
        ComputerError::BootFailed(m) | ComputerError::ExecutionFailed(m) => ApiError::Internal(m),
        ComputerError::AmbiguousOutcome(m) => ApiError::Internal(m),
        ComputerError::Cancelled => ApiError::Internal("cancelled".into()),
    }
}

fn workspace_list_path(query: &WorkspacePathQuery) -> Result<String, ApiError> {
    let path = query.path.as_deref().unwrap_or("/workspace");
    validate_workspace_list_path(path).map_err(map_workspace_validation)
}

fn workspace_read_path(query: &WorkspacePathQuery) -> Result<String, ApiError> {
    let path = query.path.as_deref().unwrap_or("/workspace");
    validate_workspace_readable_path(path).map_err(map_workspace_validation)
}

fn sort_workspace_entries(
    entries: Vec<agent_core::WorkspaceEntry>,
) -> Vec<agent_core::WorkspaceEntry> {
    let mut entries = filter_workspace_listing(entries);
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
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
    let entries = computer.list_dir(&path).await.map_err(map_computer_error)?;

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
        return Err(ApiError::Validation(
            "path must be a file, not a directory".into(),
        ));
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
    let from =
        validate_workspace_mutation_path(body.path.trim()).map_err(map_workspace_validation)?;
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

/// Owner-only: wipe durable browser sign-in state for this computer (host volume + guest profile).
pub async fn reset_browser_sign_in(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(computer_id): Path<String>,
) -> Result<axum::http::StatusCode, ApiError> {
    if !state.config.browser_enabled {
        return Err(ApiError::Conflict("Browser automation is disabled".into()));
    }
    crate::browser_profile::reset_profile_for_computer(
        &state.pool,
        &state.config,
        principal.owner_id(),
        &computer_id,
    )
    .await?;

    if state.config.browser_profiles_dir.is_some() {
        let sprite = state
            .computer_registry
            .connect_sprite(
                &state.config,
                &state.pool,
                principal.owner_id(),
                &computer_id,
                true,
            )
            .await?;
        sprite_computer::clear_guest_profile(
            sprite.client(),
            &sprite_computer::default_deny_network_policy(),
            Duration::from_secs(45),
        )
        .await
        .map_err(map_computer_error)?;
        state
            .computer_registry
            .evict(principal.owner_id(), &computer_id);
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
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
