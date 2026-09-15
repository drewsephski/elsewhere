use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const DEFAULT_API_BASE: &str = "https://api.sprites.dev/v1";
pub const DEFAULT_WORKSPACE_ROOT: &str = "/workspace";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpriteInfo {
    pub id: String,
    pub name: String,
    pub organization: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
}

#[derive(Debug, Error)]
pub enum SpriteError {
    #[error("sprite not found")]
    NotFound,
    #[error("configuration error: {0}")]
    Config(String),
    #[error("network failure: {0}")]
    Network(String),
    #[error("request timeout")]
    Timeout,
    #[error("provider error ({status}): {message}")]
    Provider { status: u16, message: String },
    #[error("malformed provider response: {0}")]
    MalformedResponse(String),
    #[error("sprite not ready: {0}")]
    NotReady(String),
}

impl SpriteError {
    pub fn sanitize_message(message: &str, token: &str) -> String {
        let mut out = message.replace(token, "[redacted]");
        if let Some(rest) = token.strip_prefix("Bearer ") {
            out = out.replace(rest, "[redacted]");
        }
        out = out.replace("Bearer ", "Bearer [redacted]");
        out
    }
}

/// Stable Fly resource name for an Elsewhere sandbox (not equal to bot display name).
pub fn sprite_name_for_sandbox(sandbox_id: &str) -> String {
    format!("elsewhere-{}", sanitize_sprite_name(sandbox_id))
}

pub fn sanitize_sprite_name(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars().take(48) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if ch == '-' || ch == '_' {
            out.push('-');
        } else {
            out.push('-');
        }
    }
    while out.contains("--") {
        out = out.replace("--", "-");
    }
    out.trim_matches('-').to_string()
}

#[derive(Debug, Deserialize)]
pub(crate) struct SpriteRecord {
    pub id: String,
    pub name: String,
    pub organization: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FsListResponse {
    pub entries: Vec<FsEntry>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FsEntry {
    pub name: String,
    pub path: String,
    #[serde(rename = "isDir", alias = "is_dir")]
    pub is_dir: bool,
    #[serde(default)]
    pub r#type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CheckpointsListResponse {
    #[serde(default)]
    pub checkpoints: Vec<CheckpointRecord>,
}

#[derive(Debug, Deserialize)]
pub struct CheckpointRecord {
    pub id: String,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default, rename = "createdAt", alias = "created_at")]
    pub created_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExecHttpJson {
    pub stdout: String,
    pub stderr: String,
    #[serde(rename = "exitCode", alias = "exit_code")]
    pub exit_code: i32,
}

#[derive(Debug, Serialize)]
pub(crate) struct CreateSpriteBody<'a> {
    pub name: &'a str,
}

#[derive(Debug, Serialize)]
pub(crate) struct CheckpointCreateBody<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<&'a str>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkPolicyBody {
    pub rules: Vec<NetworkPolicyRuleBody>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkPolicyRuleBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct StreamLine {
    pub r#type: String,
    #[serde(default)]
    pub data: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}
