pub fn redact_secrets(input: &str) -> String {
    let mut out = input.to_string();
    for secret in [
        "OPENAI_API_KEY",
        "SPRITE_TOKEN",
        "SPRITES_TOKEN",
        "ELSEWHERE_CLOUD_API_TOKEN",
        "ELSEWHERE_CONNECTOR_SECRET_KEY",
        "GITHUB_CLIENT_SECRET",
        "DATABASE_URL",
    ] {
        if out.contains(secret) {
            out = out.replace(secret, "[redacted]");
        }
    }
    if let Ok(v) = std::env::var("OPENAI_API_KEY") {
        if !v.is_empty() {
            out = out.replace(&v, "[redacted]");
        }
    }
    if let Ok(v) = std::env::var("SPRITE_TOKEN").or_else(|_| std::env::var("SPRITES_TOKEN")) {
        if !v.is_empty() {
            out = out.replace(&v, "[redacted]");
        }
    }
    if let Ok(v) = std::env::var("ELSEWHERE_CLOUD_API_TOKEN") {
        if !v.is_empty() {
            out = out.replace(&v, "[redacted]");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_text_never_contains_secret_names() {
        let msg = redact_secrets(
            "failure OPENAI_API_KEY leaked SPRITE_TOKEN ELSEWHERE_CLOUD_API_TOKEN",
        );
        assert!(!msg.contains("OPENAI_API_KEY"));
        assert!(!msg.contains("SPRITE_TOKEN"));
        assert!(!msg.contains("ELSEWHERE_CLOUD_API_TOKEN"));
    }
}
