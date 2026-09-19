use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agent_core::{AgentConnectors, ConnectorError};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::{json, Value};
use sqlx::PgPool;

use super::db::{
    load_github_credential, mark_reconnect_required, replace_connector_secret,
    GitHubCredentialLoad, PROVIDER_GITHUB,
};
use super::github_client::{
    GitHubClient, GitHubCredential, GitHubInstallation, GitHubRefreshError,
};
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
use crate::redact::redact_secrets;
use agent_core::{
    truncate_connector_tool_result, ConnectorToolDefinition, MAX_CONNECTED_APP_DESCRIPTION_CHARS,
    MAX_CONNECTED_APP_SEARCH_LIMIT,
};
use uuid::Uuid;

const CATALOG_TTL: Duration = Duration::from_secs(30);
const UNAUTHORIZED_REPO_MESSAGE: &str =
    "GitHub repository is not in the authorized installation set";

#[derive(Clone)]
pub(crate) struct AuthorizedRepository {
    owner: String,
    name: String,
    full_name: String,
    description: Option<String>,
    private: bool,
    payload: Value,
}

#[derive(Clone)]
struct CachedCatalog {
    fetched_at: Instant,
    repos: Vec<AuthorizedRepository>,
}

pub struct PostgresAgentConnectors {
    pool: PgPool,
    secret_box: Arc<ConnectorSecretBox>,
    github: GitHubClient,
    remote: RemoteHttpClient,
    catalogs: Mutex<HashMap<String, CachedCatalog>>,
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
            catalogs: Mutex::new(HashMap::new()),
        })
    }

    pub fn invalidate_github_catalog(&self, owner_id: &str) {
        if let Ok(mut catalogs) = self.catalogs.lock() {
            catalogs.remove(owner_id);
        }
    }

    pub(crate) async fn load_authorized_catalog(
        &self,
        owner_id: &str,
    ) -> Result<Vec<AuthorizedRepository>, ConnectorError> {
        let token = self.github_access_token(owner_id).await?;
        self.authorized_catalog(owner_id, &token).await
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
        let token = self.github_access_token(owner_id).await?;
        let catalog = self.authorized_catalog(owner_id, &token).await?;
        dispatch_github_tool(&self.github, &token, &catalog, tool_name, arguments).await
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

impl PostgresAgentConnectors {
    async fn github_access_token(&self, owner_id: &str) -> Result<String, ConnectorError> {
        let loaded = load_github_credential(&self.pool, owner_id, &self.secret_box)
            .await
            .map_err(|e| ConnectorError::Internal(e.to_string()))?;
        match loaded {
            GitHubCredentialLoad::Missing => Err(ConnectorError::NotConnected),
            GitHubCredentialLoad::ReconnectRequired => Err(ConnectorError::ReconnectRequired),
            GitHubCredentialLoad::Legacy => {
                let _ = mark_reconnect_required(&self.pool, owner_id, PROVIDER_GITHUB).await;
                self.invalidate_github_catalog(owner_id);
                Err(ConnectorError::ReconnectRequired)
            }
            GitHubCredentialLoad::App(credential) => {
                self.refresh_if_needed(owner_id, credential).await
            }
        }
    }

    async fn refresh_if_needed(
        &self,
        owner_id: &str,
        credential: GitHubCredential,
    ) -> Result<String, ConnectorError> {
        let now = Utc::now();
        if credential.access_token_is_fresh(now) {
            return Ok(credential.access_token);
        }
        if !credential.refresh_token_is_valid(now) {
            let _ = mark_reconnect_required(&self.pool, owner_id, PROVIDER_GITHUB).await;
            self.invalidate_github_catalog(owner_id);
            return Err(ConnectorError::ReconnectRequired);
        }
        match self
            .github
            .refresh_user_token(&credential.refresh_token)
            .await
        {
            Ok(token) => {
                let next = GitHubCredential::from_user_token(token, Utc::now());
                let plaintext = next.to_plaintext().map_err(ConnectorError::Internal)?;
                replace_connector_secret(
                    &self.pool,
                    owner_id,
                    PROVIDER_GITHUB,
                    &plaintext,
                    &self.secret_box,
                )
                .await
                .map_err(|e| ConnectorError::Internal(e.to_string()))?;
                Ok(next.access_token)
            }
            Err(GitHubRefreshError::InvalidGrant) | Err(GitHubRefreshError::NotConfigured) => {
                let _ = mark_reconnect_required(&self.pool, owner_id, PROVIDER_GITHUB).await;
                self.invalidate_github_catalog(owner_id);
                Err(ConnectorError::ReconnectRequired)
            }
            Err(GitHubRefreshError::Provider(message)) => {
                Err(ConnectorError::Provider(redact_secrets(&message)))
            }
        }
    }

    async fn authorized_catalog(
        &self,
        owner_id: &str,
        token: &str,
    ) -> Result<Vec<AuthorizedRepository>, ConnectorError> {
        if let Ok(catalogs) = self.catalogs.lock() {
            if let Some(cached) = catalogs.get(owner_id) {
                if cached.fetched_at.elapsed() < CATALOG_TTL {
                    return Ok(cached.repos.clone());
                }
            }
        }
        let repos = fetch_authorized_repositories(&self.github, token).await?;
        if let Ok(mut catalogs) = self.catalogs.lock() {
            catalogs.insert(
                owner_id.to_string(),
                CachedCatalog {
                    fetched_at: Instant::now(),
                    repos: repos.clone(),
                },
            );
        }
        Ok(repos)
    }

    pub async fn assert_repo_authorized(
        &self,
        owner_id: &str,
        owner: &str,
        repo: &str,
    ) -> Result<(), ConnectorError> {
        let token = self.github_access_token(owner_id).await?;
        let catalog = self.authorized_catalog(owner_id, &token).await?;
        require_authorized_repo(&catalog, owner, repo)?;
        Ok(())
    }

    /// Fresh installation catalog lookup for mutation boundaries (bypasses TTL cache).
    pub async fn assert_repo_authorized_fresh(
        &self,
        owner_id: &str,
        owner: &str,
        repo: &str,
    ) -> Result<(), ConnectorError> {
        self.invalidate_owner_catalog(owner_id);
        self.assert_repo_authorized(owner_id, owner, repo).await
    }

    fn invalidate_owner_catalog(&self, owner_id: &str) {
        if let Ok(mut catalogs) = self.catalogs.lock() {
            catalogs.remove(owner_id);
        }
    }

    pub async fn github_access_token_for_owner(
        &self,
        owner_id: &str,
    ) -> Result<String, ConnectorError> {
        self.github_access_token(owner_id).await
    }
}

pub(crate) async fn fetch_authorized_repositories(
    github: &GitHubClient,
    token: &str,
) -> Result<Vec<AuthorizedRepository>, ConnectorError> {
    let installations = github
        .list_user_installations(token)
        .await
        .map_err(|e| ConnectorError::Provider(redact_secrets(&e)))?;
    collect_authorized_repositories(github, token, &installations).await
}

pub(crate) async fn collect_authorized_repositories(
    github: &GitHubClient,
    token: &str,
    installations: &[GitHubInstallation],
) -> Result<Vec<AuthorizedRepository>, ConnectorError> {
    let mut repos = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for installation in installations {
        let values = github
            .list_installation_repositories(token, installation.id)
            .await
            .map_err(|e| ConnectorError::Provider(redact_secrets(&e)))?;
        for payload in values {
            let parsed = match authorized_repository_from_payload(payload) {
                Some(repo) => repo,
                None => continue,
            };
            let key = repo_key(&parsed.owner, &parsed.name);
            if seen.insert(key) {
                repos.push(parsed);
            }
        }
    }
    Ok(repos)
}

fn authorized_repository_from_payload(payload: Value) -> Option<AuthorizedRepository> {
    let full_name = payload
        .get("full_name")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            let owner = payload
                .get("owner")
                .and_then(|v| v.get("login"))
                .and_then(|v| v.as_str())?;
            let name = payload.get("name").and_then(|v| v.as_str())?;
            Some(format!("{owner}/{name}"))
        })?;
    let (owner, name) = split_full_name(&full_name)?;
    Some(AuthorizedRepository {
        owner,
        name,
        full_name,
        description: payload
            .get("description")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        private: payload
            .get("private")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        payload,
    })
}

fn split_full_name(full_name: &str) -> Option<(String, String)> {
    let (owner, name) = full_name.split_once('/')?;
    if owner.is_empty() || name.is_empty() {
        return None;
    }
    Some((owner.to_string(), name.to_string()))
}

fn repo_key(owner: &str, name: &str) -> String {
    format!(
        "{} / {}",
        owner.to_ascii_lowercase(),
        name.to_ascii_lowercase()
    )
}

impl AuthorizedRepository {
    pub(crate) fn matches(&self, owner: &str, name: &str) -> bool {
        repo_key(&self.owner, &self.name) == repo_key(owner, name)
    }
}

fn find_authorized_repo<'a>(
    catalog: &'a [AuthorizedRepository],
    owner: &str,
    repo: &str,
) -> Option<&'a AuthorizedRepository> {
    let key = repo_key(owner, repo);
    catalog
        .iter()
        .find(|item| repo_key(&item.owner, &item.name) == key)
}

fn require_authorized_repo<'a>(
    catalog: &'a [AuthorizedRepository],
    owner: &str,
    repo: &str,
) -> Result<&'a AuthorizedRepository, ConnectorError> {
    find_authorized_repo(catalog, owner, repo)
        .ok_or_else(|| ConnectorError::Provider(UNAUTHORIZED_REPO_MESSAGE.into()))
}

fn paginate<T>(items: &[T], per_page: u32, page: u32) -> &[T] {
    let per_page = per_page.clamp(1, 100) as usize;
    let page = page.max(1) as usize;
    let start = (page - 1).saturating_mul(per_page);
    if start >= items.len() {
        return &[];
    }
    let end = (start + per_page).min(items.len());
    &items[start..end]
}

fn repo_matches_query(repo: &AuthorizedRepository, query: &str) -> bool {
    let q = query.to_ascii_lowercase();
    repo.full_name.to_ascii_lowercase().contains(&q)
        || repo.name.to_ascii_lowercase().contains(&q)
        || repo.owner.to_ascii_lowercase().contains(&q)
        || repo
            .description
            .as_deref()
            .unwrap_or("")
            .to_ascii_lowercase()
            .contains(&q)
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
    catalog: &[AuthorizedRepository],
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
            let filtered: Vec<&AuthorizedRepository> = catalog
                .iter()
                .filter(|repo| match visibility {
                    "public" => !repo.private,
                    "private" => repo.private,
                    _ => true,
                })
                .collect();
            let page_items = paginate(&filtered, per_page, page);
            let repositories: Vec<Value> =
                page_items.iter().map(|repo| repo.payload.clone()).collect();
            Ok(json!({ "ok": true, "repositories": repositories }))
        }
        "github_search_repositories" => {
            let query = required_str(args, "query")?;
            let per_page = args.get("perPage").and_then(|v| v.as_u64()).unwrap_or(30) as u32;
            let page = args.get("page").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
            let matches: Vec<&AuthorizedRepository> = catalog
                .iter()
                .filter(|repo| repo_matches_query(repo, query))
                .collect();
            let page_items = paginate(&matches, per_page, page);
            let items: Vec<Value> = page_items.iter().map(|repo| repo.payload.clone()).collect();
            Ok(json!({
                "ok": true,
                "result": {
                    "total_count": matches.len(),
                    "incomplete_results": false,
                    "items": items
                }
            }))
        }
        "github_get_repository" => {
            let owner = required_str(args, "owner")?;
            let repo = required_str(args, "repo")?;
            require_authorized_repo(catalog, owner, repo)?;
            let repository = github
                .get_repository(token, owner, repo)
                .await
                .map_err(|e| ConnectorError::Provider(redact_secrets(&e)))?;
            Ok(json!({ "ok": true, "repository": repository }))
        }
        "github_get_file_contents" => {
            let owner = required_str(args, "owner")?;
            let repo = required_str(args, "repo")?;
            let path = required_str(args, "path")?;
            require_authorized_repo(catalog, owner, repo)?;
            let git_ref = args.get("ref").and_then(|v| v.as_str());
            let file = github
                .get_file_contents(token, owner, repo, path, git_ref)
                .await
                .map_err(|e| ConnectorError::Provider(redact_secrets(&e)))?;
            Ok(file)
        }
        "github_list_issues" => {
            let owner = required_str(args, "owner")?;
            let repo = required_str(args, "repo")?;
            require_authorized_repo(catalog, owner, repo)?;
            let state = args.get("state").and_then(|v| v.as_str()).unwrap_or("open");
            let per_page = args.get("perPage").and_then(|v| v.as_u64()).unwrap_or(30) as u32;
            let page = args.get("page").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
            let issues = github
                .list_issues(token, owner, repo, state, per_page, page)
                .await
                .map_err(|e| ConnectorError::Provider(redact_secrets(&e)))?;
            Ok(json!({ "ok": true, "issues": issues }))
        }
        "github_get_issue" => {
            let owner = required_str(args, "owner")?;
            let repo = required_str(args, "repo")?;
            require_authorized_repo(catalog, owner, repo)?;
            let number = required_u64(args, "number")?;
            let issue = github
                .get_issue(token, owner, repo, number)
                .await
                .map_err(|e| ConnectorError::Provider(redact_secrets(&e)))?;
            Ok(json!({ "ok": true, "issue": issue }))
        }
        "github_list_pull_requests" => {
            let owner = required_str(args, "owner")?;
            let repo = required_str(args, "repo")?;
            require_authorized_repo(catalog, owner, repo)?;
            let state = args.get("state").and_then(|v| v.as_str()).unwrap_or("open");
            let per_page = args.get("perPage").and_then(|v| v.as_u64()).unwrap_or(30) as u32;
            let page = args.get("page").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
            let pulls = github
                .list_pull_requests(token, owner, repo, state, per_page, page)
                .await
                .map_err(|e| ConnectorError::Provider(redact_secrets(&e)))?;
            Ok(json!({ "ok": true, "pullRequests": pulls }))
        }
        "github_get_pull_request" => {
            let owner = required_str(args, "owner")?;
            let repo = required_str(args, "repo")?;
            require_authorized_repo(catalog, owner, repo)?;
            let number = required_u64(args, "number")?;
            let pull = github
                .get_pull_request(token, owner, repo, number)
                .await
                .map_err(|e| ConnectorError::Provider(redact_secrets(&e)))?;
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
