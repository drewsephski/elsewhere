use std::time::Duration;

use serde_json::json;

use crate::client::CodexAppServerClient;
use crate::error::CodexProviderError;
use crate::process::DEFAULT_REQUEST_TIMEOUT;
use crate::protocol::rpc::IncomingMessage;
use crate::protocol::{
    parse_login_completed_notification, parse_login_start_response, CodexLoginHandle,
};

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexDeviceLoginHandle {
    pub login_id: String,
    pub verification_url: String,
    pub user_code: String,
}

pub fn parse_device_login(
    value: serde_json::Value,
) -> Result<CodexDeviceLoginHandle, CodexProviderError> {
    if value.get("type").and_then(|v| v.as_str()) != Some("chatgptDeviceCode") {
        return Err(CodexProviderError::Login(
            "Device sign-in requires a supported Codex version".into(),
        ));
    }
    let handle: CodexDeviceLoginHandle = serde_json::from_value(value)?;
    if handle.login_id.is_empty()
        || handle.user_code.is_empty()
        || !handle
            .verification_url
            .starts_with("https://auth.openai.com/")
    {
        return Err(CodexProviderError::Login(
            "Invalid device sign-in response".into(),
        ));
    }
    Ok(handle)
}

impl CodexAppServerClient {
    /// Supported device authorization works with a remote always-on runner.
    pub async fn start_chatgpt_device_login(
        &self,
    ) -> Result<CodexDeviceLoginHandle, CodexProviderError> {
        let result = self
            .process()
            .request(
                "account/login/start",
                json!({ "type": "chatgptDeviceCode" }),
                DEFAULT_REQUEST_TIMEOUT,
            )
            .await?;
        parse_device_login(result)
    }
    pub async fn start_chatgpt_login(&self) -> Result<CodexLoginHandle, CodexProviderError> {
        let result = self
            .process()
            .request(
                "account/login/start",
                json!({ "type": "chatgpt" }),
                DEFAULT_REQUEST_TIMEOUT,
            )
            .await?;
        parse_login_start_response(result)
    }

    pub async fn cancel_login(&self, login_id: &str) -> Result<(), CodexProviderError> {
        self.process()
            .request(
                "account/login/cancel",
                json!({ "loginId": login_id }),
                DEFAULT_REQUEST_TIMEOUT,
            )
            .await?;
        Ok(())
    }

    pub async fn wait_for_login(
        &self,
        login_id: &str,
        timeout: Duration,
    ) -> Result<CodexLoginHandle, CodexProviderError> {
        let mut notifications = self.notifications();
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(CodexProviderError::Timeout(
                    "account/login/completed".into(),
                ));
            }
            match tokio::time::timeout(remaining, notifications.recv()).await {
                Ok(Ok(IncomingMessage::Notification { method, params })) => {
                    if method != "account/login/completed" {
                        continue;
                    }
                    let (notified_id, success, error) = parse_login_completed_notification(params)?;
                    if notified_id.as_deref() != Some(login_id) {
                        continue;
                    }
                    if !success {
                        return Err(CodexProviderError::Login(
                            error.unwrap_or_else(|| "login failed".into()),
                        ));
                    }
                    let account = self.account().await?;
                    match account.account {
                        crate::protocol::CodexAccountKind::ChatGpt { .. } => {
                            return Ok(CodexLoginHandle {
                                login_id: login_id.to_string(),
                                auth_url: String::new(),
                            });
                        }
                        _ => {
                            return Err(CodexProviderError::Login(
                                "login completed but ChatGPT account not active".into(),
                            ));
                        }
                    }
                }
                Ok(Ok(_)) => continue,
                Ok(Err(_)) => {
                    return Err(CodexProviderError::Process(
                        "notification channel closed".into(),
                    ));
                }
                Err(_) => {
                    return Err(CodexProviderError::Timeout(
                        "account/login/completed".into(),
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_device_authorization_challenge() {
        let challenge = json!({"type":"chatgptDeviceCode", "loginId":"login", "userCode":"ABCD", "verificationUrl":"https://auth.openai.com/codex/device"});
        assert_eq!(
            parse_device_login(challenge.clone()).unwrap().user_code,
            "ABCD"
        );
        let mut invalid = challenge;
        invalid["verificationUrl"] = json!("https://auth.openai.com.evil.test/device");
        assert!(parse_device_login(invalid).is_err());
        assert!(parse_device_login(json!({"type":"chatgpt"})).is_err());
    }
}
