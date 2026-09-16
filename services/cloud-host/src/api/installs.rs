use axum::extract::{Path, State};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::auth::Principal;
use crate::connectors::installs::{
    delete_install, get_install, insert_install, list_installs, list_tools,
    replace_discovered_tools, store_secret, update_install_status, InstallRow, StoredSecret,
};
use crate::connectors::mcp_client::{discover_mcp_tools, drafts_from_discovered, McpAuth};
use crate::connectors::mcp_oauth::{complete_mcp_oauth, start_mcp_oauth, PendingOAuthConfig};
use crate::connectors::openapi::import_openapi;
use crate::connectors::remote::{RemoteHttpClient, RemotePolicy};
use crate::connectors::ConnectorSecretBox;
use crate::error::ApiError;
use std::sync::Arc;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallSummary {
    pub id: String,
    pub kind: String,
    pub display_name: String,
    pub endpoint_url: String,
    pub status: String,
    pub enabled: bool,
    pub tool_count: i64,
    pub sample_tools: Vec<String>,
    pub last_discovery_error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl InstallSummary {
    async fn from_row(state: &AppState, row: InstallRow) -> Result<Self, ApiError> {
        let tools = list_tools(&state.pool, row.id).await?;
        Ok(Self {
            id: row.id.to_string(),
            kind: row.kind,
            display_name: row.display_name,
            endpoint_url: row.endpoint_url,
            status: row.status,
            enabled: row.enabled,
            tool_count: tools.len() as i64,
            sample_tools: tools
                .iter()
                .take(8)
                .map(|t| t.remote_name.clone())
                .collect(),
            last_discovery_error: row.last_discovery_error,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

fn secret_box(state: &AppState) -> Result<Arc<ConnectorSecretBox>, ApiError> {
    state
        .connector_secret_box()
        .ok_or_else(|| ApiError::Validation("connectors are not configured on this host".into()))
}

fn remote_client() -> RemoteHttpClient {
    if cfg!(any(test, feature = "test-utils")) {
        RemoteHttpClient::new(RemotePolicy::for_tests())
    } else {
        RemoteHttpClient::new(RemotePolicy::production())
    }
}

fn oauth_redirect_uri(state: &AppState) -> Result<String, ApiError> {
    let origin = state.config.cors_web_origin.as_deref().ok_or_else(|| {
        ApiError::Validation("ELSEWHERE_WEB_ORIGIN is required for MCP OAuth".into())
    })?;
    Ok(format!(
        "{}/app/connectors/mcp/callback",
        origin.trim_end_matches('/')
    ))
}

pub async fn list_installs_handler(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
) -> Result<Json<Vec<InstallSummary>>, ApiError> {
    let rows = list_installs(&state.pool, owner.owner_id()).await?;
    let mut out = Vec::new();
    for row in rows {
        out.push(InstallSummary::from_row(&state, row).await?);
    }
    Ok(Json(out))
}

pub async fn get_install_handler(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> Result<Json<InstallSummary>, ApiError> {
    let row = get_install(&state.pool, owner.owner_id(), id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(InstallSummary::from_row(&state, row).await?))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMcpRequest {
    pub name: String,
    pub server_url: String,
    #[serde(default)]
    pub auth: Option<String>,
    #[serde(default)]
    pub token: Option<String>,
}

pub async fn create_mcp(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Json(body): Json<CreateMcpRequest>,
) -> Result<Json<Value>, ApiError> {
    let secret_box = secret_box(&state)?;
    let name = body.name.trim();
    if name.is_empty() || name.len() > 80 {
        return Err(ApiError::Validation("name is required".into()));
    }
    let auth = body.auth.as_deref().unwrap_or("none");
    let client = remote_client();
    client
        .validate_url(&body.server_url)
        .await
        .map_err(|e| ApiError::Validation(e.message()))?;
    match auth {
        "oauth" => {
            let start = start_mcp_oauth(
                &state.pool,
                secret_box.as_ref(),
                &client,
                owner.owner_id(),
                None,
                PendingOAuthConfig {
                    display_name: name.to_string(),
                    endpoint_url: body.server_url.clone(),
                    kind: "mcp".into(),
                    redirect_uri: oauth_redirect_uri(&state)?,
                    client_id: None,
                    authorization_endpoint: None,
                    token_endpoint: None,
                    resource: None,
                },
                None,
            )
            .await?;
            Ok(Json(json!({
                "authorizationUrl": start.authorize_url,
                "state": start.state,
                "expiresAt": start.expires_at,
                "status": "oauth_required"
            })))
        }
        "none" | "bearer" => {
            let token = body.token.as_deref().filter(|s| !s.is_empty());
            if auth == "bearer" && token.is_none() {
                return Err(ApiError::Validation("token is required".into()));
            }
            let discovery = discover_mcp_tools(
                &client,
                &body.server_url,
                &McpAuth {
                    bearer: token.map(str::to_string),
                },
            )
            .await
            .map_err(map_connector)?;
            if discovery.oauth_required {
                return Err(ApiError::Validation(
                    "this MCP server requires OAuth; choose OAuth authentication".into(),
                ));
            }
            let secret = token.map(StoredSecret::bearer);
            let row = insert_install(
                &state.pool,
                owner.owner_id(),
                "mcp",
                name,
                &body.server_url,
                &json!({ "auth": auth }),
                "connected",
                secret.as_ref(),
                secret_box.as_ref(),
                &drafts_from_discovered(&discovery.tools),
            )
            .await?;
            Ok(Json(json!(InstallSummary::from_row(&state, row).await?)))
        }
        _ => Err(ApiError::Validation(
            "auth must be none, bearer, or oauth".into(),
        )),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOpenApiRequest {
    pub name: String,
    pub openapi_url: String,
    #[serde(default)]
    pub auth: Option<String>,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub header_name: Option<String>,
}

pub async fn create_openapi(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Json(body): Json<CreateOpenApiRequest>,
) -> Result<Json<InstallSummary>, ApiError> {
    let secret_box = secret_box(&state)?;
    let name = body.name.trim();
    if name.is_empty() || name.len() > 80 {
        return Err(ApiError::Validation("name is required".into()));
    }
    let client = remote_client();
    let imported = import_openapi(&client, &body.openapi_url)
        .await
        .map_err(map_connector)?;
    let auth = body.auth.as_deref().unwrap_or("none");
    let secret = match auth {
        "none" => None,
        "bearer" => {
            Some(StoredSecret::bearer(body.token.as_deref().ok_or_else(
                || ApiError::Validation("token is required".into()),
            )?))
        }
        "api_key_header" => Some(StoredSecret::api_key_header(
            body.header_name.as_deref().ok_or_else(|| {
                ApiError::Validation("headerName is required for API key auth".into())
            })?,
            body.token
                .as_deref()
                .ok_or_else(|| ApiError::Validation("token is required".into()))?,
        )),
        _ => {
            return Err(ApiError::Validation(
                "auth must be none, bearer, or api_key_header".into(),
            ))
        }
    };
    let row = insert_install(
        &state.pool,
        owner.owner_id(),
        "openapi",
        name,
        &imported.base_url,
        &json!({
            "auth": auth,
            "documentUrl": body.openapi_url,
            "headerName": body.header_name
        }),
        "connected",
        secret.as_ref(),
        secret_box.as_ref(),
        &imported.tools,
    )
    .await?;
    InstallSummary::from_row(&state, row).await.map(Json)
}

pub async fn delete_install_handler(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    if !delete_install(&state.pool, owner.owner_id(), id).await? {
        return Err(ApiError::NotFound);
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnableRequest {
    pub enabled: bool,
}

pub async fn set_enabled(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<Uuid>,
    Json(body): Json<EnableRequest>,
) -> Result<Json<InstallSummary>, ApiError> {
    let status = if body.enabled {
        "connected"
    } else {
        "disabled"
    };
    let row = update_install_status(
        &state.pool,
        owner.owner_id(),
        id,
        status,
        Some(body.enabled),
        None,
    )
    .await?
    .ok_or(ApiError::NotFound)?;
    InstallSummary::from_row(&state, row).await.map(Json)
}

pub async fn rediscover(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> Result<Json<InstallSummary>, ApiError> {
    let secret_box = secret_box(&state)?;
    let row = get_install(&state.pool, owner.owner_id(), id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let client = remote_client();
    let secret =
        crate::connectors::installs::load_secret(&state.pool, id, secret_box.as_ref()).await?;
    match row.kind.as_str() {
        "mcp" => {
            let bearer = secret
                .as_ref()
                .and_then(|s| s.access_token.clone().or_else(|| s.token.clone()));
            let discovery = discover_mcp_tools(&client, &row.endpoint_url, &McpAuth { bearer })
                .await
                .map_err(map_connector)?;
            if discovery.oauth_required {
                let _ = update_install_status(
                    &state.pool,
                    owner.owner_id(),
                    id,
                    "reconnect_required",
                    None,
                    Some(Some("reconnect required".into())),
                )
                .await?;
                return Err(ApiError::Validation("reconnect required".into()));
            }
            let updated = replace_discovered_tools(
                &state.pool,
                owner.owner_id(),
                id,
                &drafts_from_discovered(&discovery.tools),
                "connected",
            )
            .await?
            .ok_or(ApiError::NotFound)?;
            InstallSummary::from_row(&state, updated).await.map(Json)
        }
        "openapi" => {
            let document_url = row
                .config
                .get("documentUrl")
                .and_then(|v| v.as_str())
                .unwrap_or(&row.endpoint_url);
            let imported = import_openapi(&client, document_url)
                .await
                .map_err(map_connector)?;
            let updated = replace_discovered_tools(
                &state.pool,
                owner.owner_id(),
                id,
                &imported.tools,
                "connected",
            )
            .await?
            .ok_or(ApiError::NotFound)?;
            InstallSummary::from_row(&state, updated).await.map(Json)
        }
        _ => Err(ApiError::Validation("unknown integration kind".into())),
    }
}

pub async fn oauth_start(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let secret_box = secret_box(&state)?;
    let row = get_install(&state.pool, owner.owner_id(), id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if row.kind != "mcp" {
        return Err(ApiError::Validation(
            "OAuth reconnect is only for MCP servers".into(),
        ));
    }
    let client = remote_client();
    let start = start_mcp_oauth(
        &state.pool,
        secret_box.as_ref(),
        &client,
        owner.owner_id(),
        Some(id),
        PendingOAuthConfig {
            display_name: row.display_name,
            endpoint_url: row.endpoint_url,
            kind: "mcp".into(),
            redirect_uri: oauth_redirect_uri(&state)?,
            client_id: None,
            authorization_endpoint: None,
            token_endpoint: None,
            resource: None,
        },
        None,
    )
    .await?;
    Ok(Json(json!({
        "authorizationUrl": start.authorize_url,
        "state": start.state,
        "expiresAt": start.expires_at
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthCompleteRequest {
    pub code: String,
    pub state: String,
}

pub async fn oauth_complete(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
    Json(body): Json<OAuthCompleteRequest>,
) -> Result<Json<InstallSummary>, ApiError> {
    let secret_box = secret_box(&state)?;
    let client = remote_client();
    let (pending, secret, install_id) = complete_mcp_oauth(
        &state.pool,
        secret_box.as_ref(),
        &client,
        owner.owner_id(),
        &body.code,
        &body.state,
    )
    .await?;
    let discovery = discover_mcp_tools(
        &client,
        &pending.endpoint_url,
        &McpAuth {
            bearer: secret.access_token.clone(),
        },
    )
    .await
    .map_err(map_connector)?;
    if discovery.tools.is_empty() {
        return Err(ApiError::Validation(
            "MCP server did not expose any tools after OAuth".into(),
        ));
    }
    let drafts = drafts_from_discovered(&discovery.tools);
    let row = if let Some(install_id) = install_id {
        store_secret(&state.pool, install_id, &secret, secret_box.as_ref()).await?;
        replace_discovered_tools(
            &state.pool,
            owner.owner_id(),
            install_id,
            &drafts,
            "connected",
        )
        .await?
        .ok_or(ApiError::NotFound)?
    } else {
        insert_install(
            &state.pool,
            owner.owner_id(),
            "mcp",
            &pending.display_name,
            &pending.endpoint_url,
            &json!({ "auth": "oauth" }),
            "connected",
            Some(&secret),
            secret_box.as_ref(),
            &drafts,
        )
        .await?
    };
    InstallSummary::from_row(&state, row).await.map(Json)
}

fn map_connector(err: agent_core::ConnectorError) -> ApiError {
    match err {
        agent_core::ConnectorError::Validation(m) => ApiError::Validation(m),
        agent_core::ConnectorError::ReconnectRequired => {
            ApiError::Validation("reconnect required".into())
        }
        agent_core::ConnectorError::NotFound => ApiError::NotFound,
        other => ApiError::Validation(other.message()),
    }
}
