//! GitHub coding workflow tools (checkout → edit → publish PR).
//! Mutations publish via the host; the model never receives OAuth tokens.

use async_trait::async_trait;
use serde_json::Value;

pub const GITHUB_OPEN_REPOSITORY_TOOL: &str = "github_open_repository";
pub const GITHUB_REVIEW_PUBLISH_TOOL: &str = "github_review_publish";
pub const GITHUB_PUBLISH_PULL_REQUEST_TOOL: &str = "github_publish_pull_request";

pub const GITHUB_CODING_TOOL_NAMES: &[&str] = &[
    GITHUB_OPEN_REPOSITORY_TOOL,
    GITHUB_REVIEW_PUBLISH_TOOL,
    GITHUB_PUBLISH_PULL_REQUEST_TOOL,
];

pub fn is_github_coding_mutation_tool(name: &str) -> bool {
    name == GITHUB_PUBLISH_PULL_REQUEST_TOOL
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GithubCodingError {
    NotConnected,
    ReconnectRequired,
    NotFound,
    Validation(String),
    Provider(String),
    Internal(String),
}

impl GithubCodingError {
    pub fn code(&self) -> &'static str {
        match self {
            GithubCodingError::NotConnected => "not_connected",
            GithubCodingError::ReconnectRequired => "reconnect_required",
            GithubCodingError::NotFound => "not_found",
            GithubCodingError::Validation(_) => "validation_error",
            GithubCodingError::Provider(_) => "provider_error",
            GithubCodingError::Internal(_) => "internal_error",
        }
    }

    pub fn message(&self) -> String {
        match self {
            GithubCodingError::NotConnected => {
                "Connect GitHub in Connectors before working on a repository.".into()
            }
            GithubCodingError::ReconnectRequired => {
                "GitHub needs to be reconnected before publishing changes.".into()
            }
            GithubCodingError::NotFound => "Repository or coding session was not found.".into(),
            GithubCodingError::Validation(m) => m.clone(),
            GithubCodingError::Provider(m) => m.clone(),
            GithubCodingError::Internal(m) => m.clone(),
        }
    }
}

/// Host implements repository checkout, review, and publish for agent runs.
#[async_trait]
pub trait AgentGithubCoding: Send + Sync {
    async fn dispatch_tool(
        &self,
        owner_id: &str,
        run_id: &str,
        request_id: &str,
        computer_id: &str,
        computer: &dyn crate::computer::AgentComputer,
        tool_name: &str,
        arguments: &Value,
    ) -> Result<Value, GithubCodingError>;

    /// After owner approval and before publish mutations, bind the approved workspace fingerprint.
    async fn confirm_publish_approval(
        &self,
        owner_id: &str,
        run_id: &str,
        computer: &dyn crate::computer::AgentComputer,
    ) -> Result<(), GithubCodingError> {
        Ok(())
    }

    /// Merge durable session fields into approval snapshots (never includes secrets).
    async fn approval_arguments(
        &self,
        _owner_id: &str,
        _run_id: &str,
        _computer: &dyn crate::computer::AgentComputer,
        _tool_name: &str,
        arguments: &Value,
    ) -> Result<Value, GithubCodingError> {
        Ok(arguments.clone())
    }
}
