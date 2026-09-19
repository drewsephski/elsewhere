use agent_core::ConnectorError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::service::PostgresAgentConnectors;
use crate::error::ApiError;

pub const UNAUTHORIZED_REPO_MESSAGE: &str =
    "GitHub repository is not in the authorized installation set";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConnectorNeedReason {
    Disconnected,
    ReconnectRequired,
    UnauthorizedRepo { owner: String, repo: String },
    EmptyAuthorization,
    HostUnconfigured,
}

impl ConnectorNeedReason {
    pub fn kind_name(&self) -> &'static str {
        match self {
            ConnectorNeedReason::Disconnected => "disconnected",
            ConnectorNeedReason::ReconnectRequired => "reconnect_required",
            ConnectorNeedReason::UnauthorizedRepo { .. } => "unauthorized_repo",
            ConnectorNeedReason::EmptyAuthorization => "empty_authorization",
            ConnectorNeedReason::HostUnconfigured => "host_unconfigured",
        }
    }

    pub fn repo_parts(&self) -> Option<(&str, &str)> {
        match self {
            ConnectorNeedReason::UnauthorizedRepo { owner, repo } => Some((owner, repo)),
            _ => None,
        }
    }

    pub fn from_stored(
        kind: &str,
        repo_owner: Option<String>,
        repo_name: Option<String>,
    ) -> Result<Self, ApiError> {
        match kind {
            "disconnected" => Ok(ConnectorNeedReason::Disconnected),
            "reconnect_required" => Ok(ConnectorNeedReason::ReconnectRequired),
            "empty_authorization" => Ok(ConnectorNeedReason::EmptyAuthorization),
            "host_unconfigured" => Ok(ConnectorNeedReason::HostUnconfigured),
            "unauthorized_repo" => match (repo_owner, repo_name) {
                (Some(owner), Some(repo)) if !owner.is_empty() && !repo.is_empty() => {
                    Ok(ConnectorNeedReason::UnauthorizedRepo { owner, repo })
                }
                _ => Err(ApiError::Internal(
                    "unauthorized_repo requires owner and repo".into(),
                )),
            },
            other => Err(ApiError::Internal(format!(
                "unknown connector need reason: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GithubAccess {
    Ready,
    Need(ConnectorNeedReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorNeedResolution {
    Connected,
    RepoAuthorized,
    Dismissed,
    Expired,
    Cancelled,
}

impl ConnectorNeedResolution {
    pub fn as_str(self) -> &'static str {
        match self {
            ConnectorNeedResolution::Connected => "connected",
            ConnectorNeedResolution::RepoAuthorized => "repo_authorized",
            ConnectorNeedResolution::Dismissed => "dismissed",
            ConnectorNeedResolution::Expired => "expired",
            ConnectorNeedResolution::Cancelled => "cancelled",
        }
    }

    pub fn from_stored(value: &str) -> Result<Self, ApiError> {
        match value {
            "connected" => Ok(ConnectorNeedResolution::Connected),
            "repo_authorized" => Ok(ConnectorNeedResolution::RepoAuthorized),
            "dismissed" => Ok(ConnectorNeedResolution::Dismissed),
            "expired" => Ok(ConnectorNeedResolution::Expired),
            "cancelled" => Ok(ConnectorNeedResolution::Cancelled),
            other => Err(ApiError::Internal(format!(
                "unknown connector need resolution: {other}"
            ))),
        }
    }

    pub fn satisfied(self) -> bool {
        matches!(
            self,
            ConnectorNeedResolution::Connected | ConnectorNeedResolution::RepoAuthorized
        )
    }
}

#[derive(Debug, Clone)]
pub struct ConnectorNeedRequest {
    pub owner_id: String,
    pub run_id: String,
    pub bot_id: String,
    pub tool_name: String,
    pub reason: ConnectorNeedReason,
    pub arguments: Value,
}

pub fn parse_bot_chat_return_to(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.contains('\0') {
        return None;
    }
    if trimmed.contains("://") || trimmed.starts_with("//") {
        return None;
    }
    if !trimmed.starts_with("/app/bots/") {
        return None;
    }
    let url = url::Url::parse(&format!("https://elsewhere.invalid{trimmed}")).ok()?;
    if url.scheme() != "https" || url.host_str() != Some("elsewhere.invalid") {
        return None;
    }
    if url.fragment().is_some() {
        return None;
    }
    let path = url.path();
    let bot_id = path.strip_prefix("/app/bots/")?;
    if bot_id.is_empty() || bot_id.contains('/') || bot_id.contains("..") {
        return None;
    }
    if !bot_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return None;
    }
    let mut keys = url
        .query_pairs()
        .map(|(k, _)| k.into_owned())
        .collect::<Vec<_>>();
    keys.sort();
    keys.dedup();
    if keys.is_empty() {
        return Some(format!("/app/bots/{bot_id}"));
    }
    if keys.len() == 1 && keys[0] == "conversation" {
        let conversation = url
            .query_pairs()
            .find(|(k, _)| k == "conversation")
            .map(|(_, v)| v.into_owned())?;
        if conversation.is_empty()
            || conversation.contains('/')
            || conversation.contains("..")
            || !conversation
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return None;
        }
        return Some(format!("/app/bots/{bot_id}?conversation={conversation}"));
    }
    None
}

pub async fn classify_github_access(
    connectors: Option<&PostgresAgentConnectors>,
    oauth_ready: bool,
    owner_id: &str,
    tool_name: &str,
    arguments: &Value,
) -> Result<GithubAccess, ConnectorError> {
    if !oauth_ready {
        return Ok(GithubAccess::Need(ConnectorNeedReason::HostUnconfigured));
    }
    let Some(connectors) = connectors else {
        return Ok(GithubAccess::Need(ConnectorNeedReason::HostUnconfigured));
    };
    let catalog = match connectors.load_authorized_catalog(owner_id).await {
        Ok(catalog) => catalog,
        Err(ConnectorError::NotConnected) => {
            return Ok(GithubAccess::Need(ConnectorNeedReason::Disconnected));
        }
        Err(ConnectorError::ReconnectRequired) => {
            return Ok(GithubAccess::Need(ConnectorNeedReason::ReconnectRequired));
        }
        Err(err) => return Err(err),
    };

    if is_catalog_list_tool(tool_name) {
        if catalog.is_empty() {
            return Ok(GithubAccess::Need(ConnectorNeedReason::EmptyAuthorization));
        }
        return Ok(GithubAccess::Ready);
    }

    if let Some((owner, repo)) = repo_target(arguments) {
        if catalog.iter().any(|item| item.matches(&owner, &repo)) {
            return Ok(GithubAccess::Ready);
        }
        return Ok(GithubAccess::Need(ConnectorNeedReason::UnauthorizedRepo {
            owner,
            repo,
        }));
    }

    Ok(GithubAccess::Ready)
}

fn is_catalog_list_tool(tool_name: &str) -> bool {
    tool_name == "github_list_repositories" || tool_name == "github_search_repositories"
}

fn repo_target(arguments: &Value) -> Option<(String, String)> {
    let owner = arguments
        .get("owner")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    let repo = arguments
        .get("repo")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    Some((owner.to_string(), repo.to_string()))
}

pub fn resolution_for_reason(reason: &ConnectorNeedReason) -> ConnectorNeedResolution {
    match reason {
        ConnectorNeedReason::Disconnected | ConnectorNeedReason::ReconnectRequired => {
            ConnectorNeedResolution::Connected
        }
        ConnectorNeedReason::UnauthorizedRepo { .. } | ConnectorNeedReason::EmptyAuthorization => {
            ConnectorNeedResolution::RepoAuthorized
        }
        ConnectorNeedReason::HostUnconfigured => ConnectorNeedResolution::Cancelled,
    }
}

pub fn connector_error_for_unsatisfied(reason: &ConnectorNeedReason) -> ConnectorError {
    match reason {
        ConnectorNeedReason::Disconnected | ConnectorNeedReason::EmptyAuthorization => {
            ConnectorError::NotConnected
        }
        ConnectorNeedReason::ReconnectRequired => ConnectorError::ReconnectRequired,
        ConnectorNeedReason::UnauthorizedRepo { .. } => {
            ConnectorError::Provider(UNAUTHORIZED_REPO_MESSAGE.into())
        }
        ConnectorNeedReason::HostUnconfigured => ConnectorError::NotConnected,
    }
}
