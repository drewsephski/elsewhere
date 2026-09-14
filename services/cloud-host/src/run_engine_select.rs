use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunEngineMode {
    Codex,
    Responses,
    Auto,
}

impl RunEngineMode {
    pub fn from_env() -> Self {
        match env::var("ELSEWHERE_RUN_ENGINE")
            .unwrap_or_else(|_| "auto".into())
            .to_ascii_lowercase()
            .as_str()
        {
            "codex" => Self::Codex,
            "responses" | "openai" | "api" => Self::Responses,
            _ => Self::Auto,
        }
    }
}

pub fn auto_fallback_from_codex_error(err: &str) -> bool {
    err.contains("codex_not_authenticated")
        || err.contains("codex_not_chatgpt")
        || err.contains("codex_not_installed")
        || err.contains("codex executable not found")
}
