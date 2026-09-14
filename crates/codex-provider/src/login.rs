use std::time::Duration;

use serde_json::json;

use crate::client::CodexAppServerClient;
use crate::error::CodexProviderError;
use crate::process::DEFAULT_REQUEST_TIMEOUT;
use crate::protocol::{
    parse_login_completed_notification, parse_login_start_response, CodexLoginHandle,
};
use crate::protocol::rpc::IncomingMessage;

impl CodexAppServerClient {
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
                    let (notified_id, success, error) =
                        parse_login_completed_notification(params)?;
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
