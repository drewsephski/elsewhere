use std::sync::Arc;

use agent_core::{AgentConnectors, ConnectorError};
use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::PgPool;

use super::db::{load_access_token, PROVIDER_GITHUB};
use super::github_client::GitHubClient;
use super::installs::{
    get_tool_for_owner, list_tools_for_owner, load_secret, store_secret, update_install_status,
    InstallRow, InstallToolRow, StoredSecret,
};
use super::json_schema::validate_args_against_schema;
use super::mcp_client::{call_mcp_tool, McpAuth};
use super::mcp_oauth::refresh_mcp_oauth;
use super::openapi::execute_openapi_operation;
use super::redact::{redact_value, secrets_from_stored};
use super::remote::{RemoteHttpClient, RemotePolicy};
use super::secret::ConnectorSecretBox;
use crate::bounded_text::truncate_utf8_bytes;
use agent_core::{
    truncate_connector_tool_result, ConnectorToolDefinition, MAX_CONNECTED_APP_DESCRIPTION_CHARS,
    MAX_CONNECTED_APP_SEARCH_LIMIT,
};
use uuid::Uuid;

pub struct PostgresAgentConnectors {
    pool: PgPool,
    secret_box: Arc<ConnectorSecretBox>,
    github: GitHubClient,
    remote: RemoteHttpClient,
}

impl PostgresAgentConnectors {
    pub fn new(
        pool: PgPool,
        secret_box: Arc<ConnectorSecretBox>,
        github: GitHubClient,
    ) -> Arc<Self> {
        Self::new_with_remote(
            pool,
            secret_box,
            github,
            RemoteHttpClient::new(if cfg!(any(test, feature = "test-utils")) {
                RemotePolicy::for_tests()
            } else {
                RemotePolicy::production()
            }),
        )
    }

    pub fn new_with_remote(
        pool: PgPool,
        secret_box: Arc<ConnectorSecretBox>,
        github: GitHubClient,
        remote: RemoteHttpClient,
    ) -> Arc<Self> {
        Arc::new(Self {
            pool,
            secret_box,
            github,
            remote,
        })
    }
}

#[async_trait]
impl AgentConnectors for PostgresAgentConnectors {
    async fn dispatch_connector_tool(
        &self,
        owner_id: &str,
        tool_name: &str,
        arguments: &Value,
    ) -> Result<Value, ConnectorError> {
        if !tool_name.starts_with("github_") {
            return Err(ConnectorError::Validation(format!(
                "unknown connector tool: {tool_name}"
            )));
        }
        let token = load_access_token(&self.pool, owner_id, PROVIDER_GITHUB, &self.secret_box)
            .await
            .map_err(|e| ConnectorError::Internal(e.to_string()))?
            .ok_or(ConnectorError::NotConnected)?;

        dispatch_github_tool(&self.github, &token, tool_name, arguments).await
    }

    async fn search_connected_app_tools(
        &self,
        owner_id: &str,
        query: Option<&str>,
        source: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Value, ConnectorError> {
        let limit = limit
            .unwrap_or(MAX_CONNECTED_APP_SEARCH_LIMIT)
            .min(MAX_CONNECTED_APP_SEARCH_LIMIT) as usize;
        let rows = list_tools_for_owner(&self.pool, owner_id)
            .await
            .map_err(|e| ConnectorError::Internal(e.to_string()))?;
        let query = query
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty());
        let source = source
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty());
        let mut tools = Vec::new();
        for (install, tool) in rows {
            if let Some(source) = &source {
                if !install.display_name.to_ascii_lowercase().contains(source) {
                    continue;
                }
            }
            if let Some(query) = &query {
                let hay = format!(
                    "{} {} {}",
                    tool.remote_name, tool.description, install.display_name
                )
                .to_ascii_lowercase();
                if !hay.contains(query) {
                    continue;
                }
            }
            tools.push(json!({
                "toolId": tool.id,
                "name": tool.remote_name,
                "source": install.display_name,
                "description": truncate_utf8_bytes(&tool.description, MAX_CONNECTED_APP_DESCRIPTION_CHARS),
                "readOnly": tool.read_only
            }));
            if tools.len() >= limit {
                break;
            }
        }
        Ok(json!({
            "ok": true,
            "tools": tools
        }))
    }

    async fn load_connected_app_tool(
        &self,
        owner_id: &str,
        tool_id: &str,
    ) -> Result<ConnectorToolDefinition, ConnectorError> {
        let (install, tool) = load_authorized_tool(&self.pool, owner_id, tool_id).await?;
        Ok(definition_from_rows(&install, &tool))
    }

    async fn execute_connected_app_tool(
        &self,
        owner_id: &str,
        tool_id: &str,
        arguments: &Value,
    ) -> Result<Value, ConnectorError> {
        let (install, tool) = load_authorized_tool(&self.pool, owner_id, tool_id).await?;
        validate_args_against_schema(&public_input_schema(&tool.input_schema), arguments)?;
        let mut secret = load_secret(&self.pool, install.id, &self.secret_box)
            .await
            .map_err(|e| ConnectorError::Internal(e.to_string()))?;
        if let Some(current) = secret.as_ref() {
            if current.kind == "oauth" {
                if current.access_token.is_none() {
                    return Err(ConnectorError::ReconnectRequired);
                }
                if token_expired(current) {
                    match refresh_mcp_oauth(&self.remote, current).await {
                        Ok(refreshed) => {
                            store_secret(&self.pool, install.id, &refreshed, &self.secret_box)
                                .await
                                .map_err(|e| ConnectorError::Internal(e.to_string()))?;
                            secret = Some(refreshed);
                        }
                        Err(ConnectorError::ReconnectRequired) => {
                            let _ = update_install_status(
                                &self.pool,
                                owner_id,
                                install.id,
                                "reconnect_required",
                                None,
                                Some(Some("reconnect required".into())),
                            )
                            .await;
                            return Err(ConnectorError::ReconnectRequired);
                        }
                        Err(err) => return Err(err),
                    }
                }
            }
        }
        let result = match install.kind.as_str() {
            "mcp" => {
                let bearer = secret
                    .as_ref()
                    .and_then(|s| s.access_token.clone().or_else(|| s.token.clone()));
                call_mcp_tool(
                    &self.remote,
                    &install.endpoint_url,
                    &McpAuth { bearer },
                    &tool.remote_name,
                    arguments,
                )
                .await?
            }
            "openapi" => {
                execute_openapi_operation(
                    &self.remote,
                    &install.endpoint_url,
                    &tool.remote_name,
                    &tool.input_schema,
                    arguments,
                    secret.as_ref(),
                )
                .await?
            }
            other => {
                return Err(ConnectorError::Validation(format!(
                    "unknown connector kind: {other}"
                )))
            }
        };
        let secrets = secrets_from_stored(secret.as_ref());
        let redacted = redact_value(&result, &secrets);
        truncate_connector_tool_result(redacted)
    }
}

async fn load_authorized_tool(
    pool: &sqlx::PgPool,
    owner_id: &str,
    tool_id: &str,
) -> Result<(InstallRow, InstallToolRow), ConnectorError> {
    let tool_id = Uuid::parse_str(tool_id).map_err(|_| ConnectorError::NotFound)?;
    let found = get_tool_for_owner(pool, owner_id, tool_id)
        .await
        .map_err(|e| ConnectorError::Internal(e.to_string()))?
        .ok_or(ConnectorError::NotFound)?;
    if !found.0.enabled || found.0.status != "connected" {
        if found.0.status == "reconnect_required" {
            return Err(ConnectorError::ReconnectRequired);
        }
        return Err(ConnectorError::NotConnected);
    }
    Ok(found)
}

fn public_input_schema(schema: &Value) -> Value {
    let mut obj = schema.as_object().cloned().unwrap_or_default();
    obj.remove("x-elsewhereMethod");
    obj.remove("x-elsewherePath");
    Value::Object(obj)
}

fn definition_from_rows(install: &InstallRow, tool: &InstallToolRow) -> ConnectorToolDefinition {
    ConnectorToolDefinition {
        id: tool.id.to_string(),
        install_id: install.id.to_string(),
        name: tool.remote_name.clone(),
        source: install.display_name.clone(),
        description: truncate_utf8_bytes(&tool.description, MAX_CONNECTED_APP_DESCRIPTION_CHARS),
        input_schema: public_input_schema(&tool.input_schema),
        read_only: tool.read_only,
        kind: install.kind.clone(),
    }
}

fn token_expired(secret: &StoredSecret) -> bool {
    match secret.expires_at {
        Some(ms) => ms <= chrono::Utc::now().timestamp_millis() + 30_000,
        None => false,
    }
}

async fn dispatch_github_tool(
    github: &GitHubClient,
    token: &str,
    tool_name: &str,
    args: &Value,
) -> Result<Value, ConnectorError> {
    match tool_name {
        "github_list_repositories" => {
            let visibility = args
                .get("visibility")
                .and_then(|v| v.as_str())
                .unwrap_or("all");
            let per_page = args.get("perPage").and_then(|v| v.as_u64()).unwrap_or(30) as u32;
            let page = args.get("page").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
            let repos = github
                .list_repositories(token, visibility, per_page, page)
                .await
                .map_err(ConnectorError::Provider)?;
            Ok(json!({ "ok": true, "repositories": repos }))
        }
        "github_search_repositories" => {
            let query = required_str(args, "query")?;
            let per_page = args.get("perPage").and_then(|v| v.as_u64()).unwrap_or(30) as u32;
            let page = args.get("page").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
            let result = github
                .search_repositories(token, query, per_page, page)
                .await
                .map_err(ConnectorError::Provider)?;
            Ok(json!({ "ok": true, "result": result }))
        }
        "github_get_repository" => {
            let owner = required_str(args, "owner")?;
            let repo = required_str(args, "repo")?;
            let repository = github
                .get_repository(token, owner, repo)
                .await
                .map_err(ConnectorError::Provider)?;
            Ok(json!({ "ok": true, "repository": repository }))
        }
        "github_get_file_contents" => {
            let owner = required_str(args, "owner")?;
            let repo = required_str(args, "repo")?;
            let path = required_str(args, "path")?;
            let git_ref = args.get("ref").and_then(|v| v.as_str());
            let file = github
                .get_file_contents(token, owner, repo, path, git_ref)
                .await
                .map_err(ConnectorError::Provider)?;
            Ok(file)
        }
        "github_list_issues" => {
            let owner = required_str(args, "owner")?;
            let repo = required_str(args, "repo")?;
            let state = args.get("state").and_then(|v| v.as_str()).unwrap_or("open");
            let per_page = args.get("perPage").and_then(|v| v.as_u64()).unwrap_or(30) as u32;
            let page = args.get("page").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
            let issues = github
                .list_issues(token, owner, repo, state, per_page, page)
                .await
                .map_err(ConnectorError::Provider)?;
            Ok(json!({ "ok": true, "issues": issues }))
        }
        "github_get_issue" => {
            let owner = required_str(args, "owner")?;
            let repo = required_str(args, "repo")?;
            let number = required_u64(args, "number")?;
            let issue = github
                .get_issue(token, owner, repo, number)
                .await
                .map_err(ConnectorError::Provider)?;
            Ok(json!({ "ok": true, "issue": issue }))
        }
        "github_list_pull_requests" => {
            let owner = required_str(args, "owner")?;
            let repo = required_str(args, "repo")?;
            let state = args.get("state").and_then(|v| v.as_str()).unwrap_or("open");
            let per_page = args.get("perPage").and_then(|v| v.as_u64()).unwrap_or(30) as u32;
            let page = args.get("page").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
            let pulls = github
                .list_pull_requests(token, owner, repo, state, per_page, page)
                .await
                .map_err(ConnectorError::Provider)?;
            Ok(json!({ "ok": true, "pullRequests": pulls }))
        }
        "github_get_pull_request" => {
            let owner = required_str(args, "owner")?;
            let repo = required_str(args, "repo")?;
            let number = required_u64(args, "number")?;
            let pull = github
                .get_pull_request(token, owner, repo, number)
                .await
                .map_err(ConnectorError::Provider)?;
            Ok(json!({ "ok": true, "pullRequest": pull }))
        }
        other => Err(ConnectorError::Validation(format!(
            "unknown connector tool: {other}"
        ))),
    }
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, ConnectorError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ConnectorError::Validation(format!("missing or empty `{key}`")))
}

fn required_u64(args: &Value, key: &str) -> Result<u64, ConnectorError> {
    args.get(key)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| ConnectorError::Validation(format!("missing `{key}`")))
}
