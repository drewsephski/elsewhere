use crate::redact::redact_secrets;

const SECRET_MARKERS: &[&str] = &[
    "authorization: bearer",
    "bearer ",
    "xoxb-",
    "xoxp-",
    "xoxa-",
    "xoxe-",
    "gho_",
    "ghp_",
    "ghu_",
    "github_pat_",
    "sk-ant-",
    "sk-proj-",
    "openai_api_key",
    "api_key=",
    "api-key:",
    "client_secret",
    "client-secret",
    "refresh_token",
    "access_token",
    "id_token",
    "sessionid=",
    "set-cookie:",
    "cookie:",
    "passwd",
    "password=",
    "password:",
    "passcode",
    "one-time code",
    "otp ",
    "totp",
    "credit card",
    "card number",
    "cvv",
    "ssn",
    "private_key",
    "begin rsa private",
    "begin openssh private",
];

/// Returns true when content looks like a secret rather than a durable fact.
pub fn looks_like_secret(content: &str) -> bool {
    let redacted = redact_secrets(content);
    if redacted.contains("[redacted]") {
        return true;
    }
    let lower = content.to_ascii_lowercase();
    SECRET_MARKERS.iter().any(|marker| lower.contains(marker))
        || looks_like_jwt(&lower)
        || looks_like_hex_blob(content)
}

pub fn reject_secret_content(content: &str) -> Result<(), String> {
    if looks_like_secret(content) {
        return Err("Memory cannot store secrets, credentials, or authentication material".into());
    }
    Ok(())
}

fn looks_like_jwt(lower: &str) -> bool {
    lower.contains("eyj") && lower.contains('.') && lower.len() > 40
}

fn looks_like_hex_blob(content: &str) -> bool {
    let compact: String = content.chars().filter(|c| !c.is_whitespace()).collect();
    compact.len() >= 48 && compact.len() <= 256 && compact.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bearer_and_slack_tokens() {
        assert!(looks_like_secret("Authorization: Bearer abc.def.ghi"));
        assert!(looks_like_secret("xoxb-123456-secret"));
        assert!(!looks_like_secret("Drew prefers pnpm for Elsewhere."));
    }
}
