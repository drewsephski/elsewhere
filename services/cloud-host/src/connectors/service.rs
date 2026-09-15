use std::sync::Arc;

use agent_core::{AgentConnectors, ConnectorError};
use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::PgPool;

use super::db::{load_access_token, PROVIDER_GITHUB};
use super::github_client::GitHubClient;
use super::secret::ConnectorSecretBox;

pub struct PostgresAgentConnectors {
    pool: PgPool,
    secret_box: Arc<ConnectorSecretBox>,
    github: GitHubClient,
}

impl PostgresAgentConnectors {
    pub fn new(pool: PgPool, secret_box: Arc<ConnectorSecretBox>, github: GitHubClient) -> Arc<Self> {
        Arc::new(Self {
            pool,
            secret_box,
            github,
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
            return Err(ConnectorError::Validation(format!("unknown connector tool: {tool_name}")));
        }
        let token = load_access_token(&self.pool, owner_id, PROVIDER_GITHUB, &self.secret_box)
            .await
            .map_err(|e| ConnectorError::Internal(e.to_string()))?
            .ok_or(ConnectorError::NotConnected)?;

        dispatch_github_tool(&self.github, &token, tool_name, arguments).await
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
        other => Err(ConnectorError::Validation(format!("unknown connector tool: {other}"))),
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
