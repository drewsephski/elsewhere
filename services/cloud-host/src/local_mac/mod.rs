//! Local Mac Companion pairing and durable device credentials.
//!
//! Raw pairing secrets and node credentials are returned to the Mac once and
//! never persisted. Postgres stores HMAC-SHA256 hashes only.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;

use crate::error::ApiError;

pub mod db;

pub const PROVIDER: &str = "local_mac";
pub const DISPLAY_NAME: &str = "This Mac";
pub const PAIRING_TTL: Duration = Duration::from_secs(10 * 60);
pub const CREDENTIAL_HINT_LEN: usize = 4;
const TOKEN_BYTES: usize = 32;
const USER_CODE_CHARS: usize = 8;
const USER_CODE_ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
const PAIRING_STARTS_PER_WINDOW: usize = 8;
const PAIRING_START_WINDOW: Duration = Duration::from_secs(15 * 60);
const MAX_PENDING_PER_INSTALLATION: i64 = 3;

const HMAC_PAIRING_SECRET: &[u8] = b"elsewhere-local-mac:pairing-secret:v1:";
const HMAC_USER_CODE: &[u8] = b"elsewhere-local-mac:user-code:v1:";
const HMAC_NODE_CREDENTIAL: &[u8] = b"elsewhere-local-mac:node-credential:v1:";

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Default)]
pub struct PairingStartLimiter {
    events: Arc<DashMap<String, Vec<Instant>>>,
}

impl PairingStartLimiter {
    pub fn check(&self, installation_id: &str) -> Result<(), ApiError> {
        let now = Instant::now();
        let mut entry = self.events.entry(installation_id.to_string()).or_default();
        entry.retain(|at| now.duration_since(*at) < PAIRING_START_WINDOW);
        if entry.len() >= PAIRING_STARTS_PER_WINDOW {
            return Err(ApiError::RateLimited(
                "too many pairing attempts from this Mac".into(),
            ));
        }
        entry.push(now);
        Ok(())
    }
}

pub fn parse_credential_key(encoded: &str) -> Result<[u8; 32], String> {
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded.trim())
        .map_err(|e| format!("invalid ELSEWHERE_LOCAL_MAC_CREDENTIAL_KEY encoding: {e}"))?;
    if bytes.len() != 32 {
        return Err("ELSEWHERE_LOCAL_MAC_CREDENTIAL_KEY must decode to 32 bytes".into());
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}

pub fn hmac_bytes(key: &[u8; 32], domain: &[u8], value: &str) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC-SHA256 accepts 32-byte keys");
    mac.update(domain);
    mac.update(value.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

pub fn hash_pairing_secret(key: &[u8; 32], secret: &str) -> Vec<u8> {
    hmac_bytes(key, HMAC_PAIRING_SECRET, secret)
}

pub fn hash_user_code(key: &[u8; 32], user_code: &str) -> Vec<u8> {
    hmac_bytes(key, HMAC_USER_CODE, &normalize_user_code(user_code))
}

pub fn hash_node_credential(key: &[u8; 32], credential: &str) -> Vec<u8> {
    hmac_bytes(key, HMAC_NODE_CREDENTIAL, credential)
}

pub fn hashes_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

pub fn generate_pairing_secret() -> String {
    encode_token()
}

pub fn generate_node_credential() -> String {
    format!("emac_{}", encode_token())
}

pub fn credential_hint(credential: &str) -> String {
    if credential.len() <= CREDENTIAL_HINT_LEN {
        credential.to_string()
    } else {
        credential[credential.len() - CREDENTIAL_HINT_LEN..].to_string()
    }
}

pub fn generate_user_code() -> String {
    let mut bytes = [0u8; USER_CODE_CHARS];
    rand::thread_rng().fill_bytes(&mut bytes);
    let mut chars = String::with_capacity(USER_CODE_CHARS + 1);
    for (index, byte) in bytes.iter().enumerate() {
        if index == USER_CODE_CHARS / 2 {
            chars.push('-');
        }
        let symbol = USER_CODE_ALPHABET[*byte as usize % USER_CODE_ALPHABET.len()];
        chars.push(symbol as char);
    }
    chars
}

pub fn normalize_user_code(raw: &str) -> String {
    raw.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_uppercase())
        .collect()
}

pub fn is_plausible_user_code(raw: &str) -> bool {
    let normalized = normalize_user_code(raw);
    normalized.len() == USER_CODE_CHARS
        && normalized.bytes().all(|b| USER_CODE_ALPHABET.contains(&b))
}

pub fn is_plausible_installation_id(raw: &str) -> bool {
    uuid::Uuid::parse_str(raw.trim()).is_ok()
}

pub fn normalize_device_name(raw: &str) -> Result<String, ApiError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 100 {
        return Err(ApiError::Validation(
            "deviceName must contain 1 to 100 bytes".into(),
        ));
    }
    Ok(trimmed.to_string())
}

pub fn verification_url(web_origin: Option<&str>, pairing_id: &str, user_code: &str) -> String {
    let encoded_id = urlencoding::encode(pairing_id);
    let encoded_code = urlencoding::encode(user_code);
    let path = format!("/pair/mac?pairingId={encoded_id}&userCode={encoded_code}");
    match web_origin.map(str::trim).filter(|value| !value.is_empty()) {
        Some(origin) => format!("{}{path}", origin.trim_end_matches('/')),
        None => path,
    }
}

pub fn require_credential_key(key: Option<&[u8; 32]>) -> Result<&[u8; 32], ApiError> {
    key.ok_or_else(|| ApiError::Internal("local Mac pairing is not configured".into()))
}

pub fn max_pending_per_installation() -> i64 {
    MAX_PENDING_PER_INSTALLATION
}

fn encode_token() -> String {
    let mut bytes = [0u8; TOKEN_BYTES];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_code_roundtrip_normalizes_hyphen_and_case() {
        let code = generate_user_code();
        assert!(is_plausible_user_code(&code));
        assert_eq!(
            normalize_user_code(&code),
            normalize_user_code(&code.to_lowercase())
        );
        assert!(code.contains('-'));
    }

    #[test]
    fn hmac_is_domain_separated() {
        let key = [7u8; 32];
        let value = "same-value";
        assert_ne!(
            hash_pairing_secret(&key, value),
            hash_node_credential(&key, value)
        );
        assert_ne!(
            hash_pairing_secret(&key, value),
            hash_user_code(&key, value)
        );
    }

    #[test]
    fn installation_id_must_be_uuid() {
        assert!(is_plausible_installation_id(
            "3fa85f64-5717-4562-b3fc-2c963f66afa6"
        ));
        assert!(!is_plausible_installation_id("not-a-uuid"));
        assert!(!is_plausible_installation_id(""));
    }
}
