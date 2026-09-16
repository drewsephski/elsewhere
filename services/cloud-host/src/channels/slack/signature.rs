//! Slack Events API request verification (HMAC-SHA256, v0).
//!
//! Official algorithm: https://docs.slack.dev/authentication/verifying-requests-from-slack
//! Base string is `v0:{timestamp}:{raw_body}` signed with the app signing secret.

use hmac::{Hmac, Mac};
use sha2::Sha256;

use super::super::SLACK_TIMESTAMP_SKEW_SECS;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlackSignatureError {
    MissingTimestamp,
    MissingSignature,
    InvalidTimestamp,
    StaleTimestamp,
    InvalidSignature,
}

impl SlackSignatureError {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MissingTimestamp => "missing timestamp",
            Self::MissingSignature => "missing signature",
            Self::InvalidTimestamp => "invalid timestamp",
            Self::StaleTimestamp => "stale timestamp",
            Self::InvalidSignature => "invalid signature",
        }
    }
}

/// Verify a Slack request using the **raw request bytes** Slack signed.
pub fn verify_slack_request(
    signing_secret: &str,
    timestamp_header: Option<&str>,
    signature_header: Option<&str>,
    raw_body: &[u8],
    now_unix: i64,
) -> Result<(), SlackSignatureError> {
    let timestamp = timestamp_header
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or(SlackSignatureError::MissingTimestamp)?;
    let signature = signature_header
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or(SlackSignatureError::MissingSignature)?;

    let ts: i64 = timestamp
        .parse()
        .map_err(|_| SlackSignatureError::InvalidTimestamp)?;
    if now_unix.abs_diff(ts) > SLACK_TIMESTAMP_SKEW_SECS as u64 {
        return Err(SlackSignatureError::StaleTimestamp);
    }

    let hex_sig = signature
        .strip_prefix("v0=")
        .ok_or(SlackSignatureError::InvalidSignature)?;
    let expected = hex::decode(hex_sig).map_err(|_| SlackSignatureError::InvalidSignature)?;

    let mut mac = HmacSha256::new_from_slice(signing_secret.as_bytes())
        .map_err(|_| SlackSignatureError::InvalidSignature)?;
    mac.update(b"v0:");
    mac.update(timestamp.as_bytes());
    mac.update(b":");
    mac.update(raw_body);
    mac.verify_slice(&expected)
        .map_err(|_| SlackSignatureError::InvalidSignature)?;
    Ok(())
}

pub fn sign_slack_request(signing_secret: &str, timestamp: &str, raw_body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(signing_secret.as_bytes()).expect("hmac key");
    mac.update(b"v0:");
    mac.update(timestamp.as_bytes());
    mac.update(b":");
    mac.update(raw_body);
    format!("v0={}", hex::encode(mac.finalize().into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_signature_over_raw_bytes() {
        let secret = "8f742231b10e8888abcd99yyyzzz85a5";
        let ts = "1531420618";
        let body = b"{\"token\":\"ignored\",\"type\":\"event_callback\"}";
        let sig = sign_slack_request(secret, ts, body);
        assert!(verify_slack_request(secret, Some(ts), Some(&sig), body, 1531420618).is_ok());
    }

    #[test]
    fn rejects_reparsed_json_that_changes_bytes() {
        let secret = "signing-secret";
        let ts = "1531420618";
        let raw = b"{\"a\":1, \"b\":2}";
        let sig = sign_slack_request(secret, ts, raw);
        let reparsed = serde_json::to_vec(&serde_json::json!({"a":1,"b":2})).unwrap();
        assert_ne!(raw.as_slice(), reparsed.as_slice());
        assert_eq!(
            verify_slack_request(secret, Some(ts), Some(&sig), &reparsed, 1531420618),
            Err(SlackSignatureError::InvalidSignature)
        );
        assert!(verify_slack_request(secret, Some(ts), Some(&sig), raw, 1531420618).is_ok());
    }

    #[test]
    fn rejects_invalid_signature() {
        let secret = "signing-secret";
        let ts = "1531420618";
        let body = b"{}";
        assert_eq!(
            verify_slack_request(secret, Some(ts), Some("v0=deadbeef"), body, 1531420618),
            Err(SlackSignatureError::InvalidSignature)
        );
    }

    #[test]
    fn rejects_stale_timestamp() {
        let secret = "signing-secret";
        let ts = "1000";
        let body = b"{}";
        let sig = sign_slack_request(secret, ts, body);
        assert_eq!(
            verify_slack_request(secret, Some(ts), Some(&sig), body, 1000 + 301),
            Err(SlackSignatureError::StaleTimestamp)
        );
    }
}
