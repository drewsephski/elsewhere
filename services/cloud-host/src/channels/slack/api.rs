//! Slack Web API client (OAuth v2, chat.postMessage, apps.uninstall).

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::channels::types::{
    DeliveryReceipt, MessagingProvider, NormalizedInbound, OutboundDelivery, ProviderDeliveryError,
};
use crate::channels::{ChannelProvider, SLACK_BOT_SCOPES};

use super::events::normalize_slack_event;

const DEFAULT_API_BASE: &str = "https://slack.com/api";
const DEFAULT_OAUTH_BASE: &str = "https://slack.com";

#[derive(Clone)]
pub struct SlackClient {
    http: reqwest::Client,
    api_base: String,
    oauth_base: String,
}

impl SlackClient {
    pub fn production() -> Self {
        Self::with_bases(DEFAULT_API_BASE.to_string(), DEFAULT_OAUTH_BASE.to_string())
    }

    pub fn with_bases(api_base: String, oauth_base: String) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .expect("slack http client"),
            api_base: api_base.trim_end_matches('/').to_string(),
            oauth_base: oauth_base.trim_end_matches('/').to_string(),
        }
    }

    pub fn authorize_url(&self, client_id: &str, redirect_uri: &str, state: &str) -> String {
        format!(
            "{}/oauth/v2/authorize?client_id={}&scope={}&redirect_uri={}&state={}",
            self.oauth_base,
            urlencoding::encode(client_id),
            urlencoding::encode(SLACK_BOT_SCOPES),
            urlencoding::encode(redirect_uri),
            urlencoding::encode(state),
        )
    }

    pub async fn exchange_code(
        &self,
        client_id: &str,
        client_secret: &str,
        code: &str,
        redirect_uri: &str,
    ) -> Result<SlackOAuthInstall, String> {
        let response = self
            .http
            .post(format!("{}/oauth.v2.access", self.api_base))
            .form(&[
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("code", code),
                ("redirect_uri", redirect_uri),
            ])
            .send()
            .await
            .map_err(|e| format!("Slack OAuth exchange failed: {e}"))?;
        let body: SlackOAuthResponse = response
            .json()
            .await
            .map_err(|e| format!("Slack OAuth response invalid: {e}"))?;
        if !body.ok {
            return Err(body.error.unwrap_or_else(|| "oauth_failed".into()));
        }
        let team = body.team.unwrap_or_default();
        let authed = body.authed_user.unwrap_or_default();
        let access_token = body
            .access_token
            .ok_or_else(|| "missing access_token".to_string())?;
        let installer = authed
            .id
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "missing installing Slack user".to_string())?;
        let workspace_id = team
            .id
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "missing Slack workspace".to_string())?;
        Ok(SlackOAuthInstall {
            access_token,
            workspace_id,
            workspace_name: team.name.filter(|s| !s.is_empty()),
            installer_user_id: installer,
            bot_user_id: body.bot_user_id.filter(|s| !s.is_empty()),
        })
    }

    pub async fn uninstall(
        &self,
        token: &str,
        client_id: &str,
        client_secret: &str,
    ) -> Result<(), String> {
        let response = self
            .http
            .post(format!("{}/apps.uninstall", self.api_base))
            .header("Authorization", format!("Bearer {token}"))
            .form(&[("client_id", client_id), ("client_secret", client_secret)])
            .send()
            .await
            .map_err(|e| format!("Slack uninstall failed: {e}"))?;
        let body: SlackOkResponse = response
            .json()
            .await
            .map_err(|e| format!("Slack uninstall response invalid: {e}"))?;
        if !body.ok {
            return Err(body.error.unwrap_or_else(|| "uninstall_failed".into()));
        }
        Ok(())
    }

    pub async fn post_message(
        &self,
        token: &str,
        channel: &str,
        text: &str,
        thread_ts: Option<&str>,
    ) -> Result<DeliveryReceipt, ProviderDeliveryError> {
        let mut payload = json!({
            "channel": channel,
            "text": text,
            "unfurl_links": false,
            "unfurl_media": false,
        });
        if let Some(thread_ts) = thread_ts {
            payload["thread_ts"] = json!(thread_ts);
        }
        let response = self
            .http
            .post(format!("{}/chat.postMessage", self.api_base))
            .header("Authorization", format!("Bearer {token}"))
            .json(&payload)
            .send()
            .await
            .map_err(|e| ProviderDeliveryError::Retryable {
                message: format!("Slack postMessage network error: {e}"),
                retry_after_secs: None,
            })?;

        let status = response.status();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        if status.as_u16() == 429 {
            return Err(ProviderDeliveryError::Retryable {
                message: "Slack rate limited".into(),
                retry_after_secs: retry_after.or(Some(1)),
            });
        }
        if status.is_server_error() {
            return Err(ProviderDeliveryError::Retryable {
                message: format!("Slack postMessage HTTP {status}"),
                retry_after_secs: retry_after,
            });
        }

        let body: SlackPostMessageResponse =
            response
                .json()
                .await
                .map_err(|e| ProviderDeliveryError::Retryable {
                    message: format!("Slack postMessage response invalid: {e}"),
                    retry_after_secs: None,
                })?;
        if !body.ok {
            return Err(classify_slack_api_error(
                body.error.as_deref().unwrap_or("unknown"),
            ));
        }
        let ts =
            body.ts
                .filter(|s| !s.is_empty())
                .ok_or_else(|| ProviderDeliveryError::Retryable {
                    message: "Slack postMessage missing ts".into(),
                    retry_after_secs: None,
                })?;
        Ok(DeliveryReceipt {
            provider_message_id: ts,
        })
    }
}

#[async_trait]
impl MessagingProvider for SlackClient {
    fn provider(&self) -> ChannelProvider {
        ChannelProvider::Slack
    }

    fn normalize_inbound(&self, envelope: &Value) -> Option<NormalizedInbound> {
        normalize_slack_event(envelope)
    }

    async fn deliver(
        &self,
        token: &str,
        message: &OutboundDelivery,
    ) -> Result<DeliveryReceipt, ProviderDeliveryError> {
        self.post_message(
            token,
            &message.channel_id,
            &message.body,
            message.thread_ts.as_deref(),
        )
        .await
    }
}

pub fn classify_slack_api_error(error: &str) -> ProviderDeliveryError {
    match error {
        "invalid_auth"
        | "not_authed"
        | "token_revoked"
        | "token_expired"
        | "account_inactive"
        | "missing_scope"
        | "org_login_required"
        | "ekm_access_denied"
        | "cannot_reply_to_broadcast"
        | "channel_not_found"
        | "is_archived"
        | "msg_too_long"
        | "invalid_arguments"
        | "restricted_action" => ProviderDeliveryError::Permanent {
            message: error.to_string(),
        },
        "ratelimited" | "rate_limited" => ProviderDeliveryError::Retryable {
            message: error.to_string(),
            retry_after_secs: Some(1),
        },
        other => ProviderDeliveryError::Retryable {
            message: other.to_string(),
            retry_after_secs: None,
        },
    }
}

#[derive(Debug, Clone)]
pub struct SlackOAuthInstall {
    pub access_token: String,
    pub workspace_id: String,
    pub workspace_name: Option<String>,
    pub installer_user_id: String,
    pub bot_user_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct SlackTeam {
    id: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct SlackAuthedUser {
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SlackOAuthResponse {
    ok: bool,
    error: Option<String>,
    access_token: Option<String>,
    bot_user_id: Option<String>,
    team: Option<SlackTeam>,
    authed_user: Option<SlackAuthedUser>,
}

#[derive(Debug, Deserialize)]
struct SlackOkResponse {
    ok: bool,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SlackPostMessageResponse {
    ok: bool,
    error: Option<String>,
    ts: Option<String>,
}
