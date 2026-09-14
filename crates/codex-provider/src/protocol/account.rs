use serde::Deserialize;
use serde_json::Value;

use crate::error::CodexProviderError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexAccountKind {
    ChatGpt {
        email: Option<String>,
        plan_type: String,
    },
    ApiKey,
    Other(String),
    NotLoggedIn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexAccountState {
    pub account: CodexAccountKind,
    pub requires_openai_auth: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexLoginHandle {
    pub login_id: String,
    pub auth_url: String,
}

#[derive(Debug, Clone)]
pub struct CodexRateLimitsSnapshot {
    pub raw: Value,
}

pub fn parse_account_response(value: Value) -> Result<CodexAccountState, CodexProviderError> {
    let requires_openai_auth = value
        .get("requiresOpenaiAuth")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let account_value = value.get("account");
    let account = match account_value {
        None | Some(Value::Null) => CodexAccountKind::NotLoggedIn,
        Some(obj) => {
            let account_type = obj
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            match account_type {
                "chatgpt" => CodexAccountKind::ChatGpt {
                    email: obj
                        .get("email")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    plan_type: obj
                        .get("planType")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                },
                "apiKey" => CodexAccountKind::ApiKey,
                other => CodexAccountKind::Other(other.to_string()),
            }
        }
    };
    Ok(CodexAccountState {
        account,
        requires_openai_auth,
    })
}

pub fn parse_login_start_response(value: Value) -> Result<CodexLoginHandle, CodexProviderError> {
    let login_type = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if login_type != "chatgpt" {
        return Err(CodexProviderError::Login(format!(
            "unexpected login start type: {login_type}"
        )));
    }
    let login_id = value
        .get("loginId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CodexProviderError::Login("missing loginId".into()))?
        .to_string();
    let auth_url = value
        .get("authUrl")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CodexProviderError::Login("missing authUrl".into()))?
        .to_string();
    Ok(CodexLoginHandle { login_id, auth_url })
}

#[derive(Debug, Deserialize)]
struct LoginCompletedParams {
    #[serde(rename = "loginId", default)]
    login_id: Option<String>,
    success: bool,
    #[serde(default)]
    error: Option<String>,
}

pub fn parse_login_completed_notification(
    params: Value,
) -> Result<(Option<String>, bool, Option<String>), CodexProviderError> {
    let parsed: LoginCompletedParams = serde_json::from_value(params)?;
    Ok((parsed.login_id, parsed.success, parsed.error))
}

pub fn parse_rate_limits_response(value: Value) -> Result<CodexRateLimitsSnapshot, CodexProviderError> {
    Ok(CodexRateLimitsSnapshot { raw: value })
}

/// Subscription runs require `account.type == chatgpt`. `requiresOpenaiAuth` is informational only.
pub fn require_chatgpt_account(state: &CodexAccountState) -> Result<String, CodexProviderError> {
    match &state.account {
        CodexAccountKind::NotLoggedIn => Err(CodexProviderError::Account(
            "codex_not_authenticated".into(),
        )),
        CodexAccountKind::ApiKey => Err(CodexProviderError::Account("codex_not_chatgpt".into())),
        CodexAccountKind::ChatGpt { plan_type, .. } => Ok(plan_type.clone()),
        CodexAccountKind::Other(kind) => Err(CodexProviderError::Account(format!(
            "codex_unsupported_account:{kind}"
        ))),
    }
}

pub fn account_auth_metadata(state: &CodexAccountState) -> (String, Option<String>) {
    match &state.account {
        CodexAccountKind::ChatGpt { plan_type, .. } => ("chatgpt".into(), Some(plan_type.clone())),
        CodexAccountKind::ApiKey => ("apiKey".into(), None),
        CodexAccountKind::NotLoggedIn => ("none".into(), None),
        CodexAccountKind::Other(kind) => (kind.clone(), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_chatgpt_account() {
        let state = parse_account_response(json!({
            "account": { "type": "chatgpt", "email": "a@b.com", "planType": "pro" },
            "requiresOpenaiAuth": false
        }))
        .unwrap();
        assert_eq!(
            state.account,
            CodexAccountKind::ChatGpt {
                email: Some("a@b.com".into()),
                plan_type: "pro".into()
            }
        );
    }

    #[test]
    fn chatgpt_account_valid_when_requires_openai_auth_true() {
        let state = parse_account_response(json!({
            "account": { "type": "chatgpt", "planType": "prolite" },
            "requiresOpenaiAuth": true
        }))
        .unwrap();
        assert_eq!(require_chatgpt_account(&state).unwrap(), "prolite");
    }

    #[test]
    fn parse_api_key_account() {
        let state = parse_account_response(json!({
            "account": { "type": "apiKey" },
            "requiresOpenaiAuth": true
        }))
        .unwrap();
        assert_eq!(state.account, CodexAccountKind::ApiKey);
    }
}
