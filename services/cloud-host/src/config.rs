use std::env;
use std::path::PathBuf;

use agent_core::DEFAULT_MODEL;

use crate::run_engine_select::RunEngineMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    InternalToken,
    Jwt,
    Hybrid,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub openai_api_key: Option<String>,
    pub sprite_token: String,
    pub api_token: String,
    pub auth_mode: AuthMode,
    pub jwt_issuer: Option<String>,
    pub jwt_audience: Option<String>,
    pub jwt_jwks_url: Option<String>,
    pub cors_web_origin: Option<String>,
    pub allow_codex_login: bool,
    pub sprites_api_base: String,
    pub max_concurrent_runs: usize,
    pub run_timeout_secs: u64,
    pub bind_addr: String,
    pub run_engine: RunEngineMode,
    pub codex_executable: Option<PathBuf>,
    pub codex_profiles_dir: Option<PathBuf>,
    pub tool_approval_timeout_secs: u64,
    pub enforce_tool_approvals_internal: bool,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let database_url = require_env("DATABASE_URL")?;
        let openai_api_key = env::var("OPENAI_API_KEY").ok().filter(|v| !v.is_empty());
        let run_engine = RunEngineMode::from_env()?;
        if run_engine == RunEngineMode::Responses && openai_api_key.is_none() {
            return Err(
                "OPENAI_API_KEY is required when ELSEWHERE_RUN_ENGINE=responses".to_string(),
            );
        }
        let sprite_token = env::var("SPRITE_TOKEN")
            .or_else(|_| env::var("SPRITES_TOKEN"))
            .map_err(|_| "SPRITE_TOKEN (or deprecated SPRITES_TOKEN) is required".to_string())?;
        let api_token = require_env("ELSEWHERE_CLOUD_API_TOKEN")?;
        let auth_mode = parse_auth_mode(
            env::var("ELSEWHERE_AUTH_MODE")
                .unwrap_or_else(|_| "hybrid".into())
                .as_str(),
        )?;
        let jwt_issuer = env::var("ELSEWHERE_JWT_ISSUER")
            .ok()
            .filter(|v| !v.is_empty());
        let jwt_audience = env::var("ELSEWHERE_JWT_AUDIENCE")
            .ok()
            .filter(|v| !v.is_empty());
        let jwt_jwks_url = env::var("ELSEWHERE_JWT_JWKS_URL")
            .ok()
            .filter(|v| !v.is_empty());
        if auth_mode == AuthMode::Jwt {
            if jwt_jwks_url.is_none() || jwt_issuer.is_none() || jwt_audience.is_none() {
                return Err(
                    "ELSEWHERE_JWT_JWKS_URL, ELSEWHERE_JWT_ISSUER, and ELSEWHERE_JWT_AUDIENCE are required when ELSEWHERE_AUTH_MODE=jwt".into(),
                );
            }
        }
        let cors_web_origin = env::var("ELSEWHERE_WEB_ORIGIN")
            .ok()
            .filter(|v| !v.is_empty());
        let allow_codex_login = env::var("ELSEWHERE_ALLOW_CODEX_LOGIN")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let max_concurrent_runs = env::var("ELSEWHERE_MAX_CONCURRENT_RUNS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);
        let run_timeout_secs = env::var("ELSEWHERE_RUN_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(15 * 60);
        let tool_approval_timeout_secs = env::var("ELSEWHERE_TOOL_APPROVAL_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5 * 60);
        let enforce_tool_approvals_internal = env::var("ELSEWHERE_ENFORCE_TOOL_APPROVALS")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let codex_executable = env::var("CODEX_EXECUTABLE")
            .ok()
            .map(PathBuf::from)
            .or_else(|| which_codex_on_path());

        Ok(Self {
            database_url,
            openai_api_key,
            sprite_token,
            api_token,
            auth_mode,
            jwt_issuer,
            jwt_audience,
            jwt_jwks_url,
            cors_web_origin,
            allow_codex_login,
            sprites_api_base: env::var("SPRITES_API_BASE")
                .unwrap_or_else(|_| sprite_computer::DEFAULT_API_BASE.to_string()),
            max_concurrent_runs,
            run_timeout_secs,
            bind_addr: env::var("ELSEWHERE_BIND").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            run_engine,
            codex_executable,
            codex_profiles_dir: env::var("ELSEWHERE_CODEX_PROFILES_DIR")
                .ok()
                .map(PathBuf::from),
            tool_approval_timeout_secs,
            enforce_tool_approvals_internal,
        })
    }

    pub fn log_summary(&self) {
        tracing::info!(
            database = true,
            openai_api_key_configured = self.openai_api_key.is_some(),
            sprite = true,
            cloud_api_auth = true,
            auth_mode = ?self.auth_mode,
            jwt_configured = self.jwt_jwks_url.is_some(),
            cors_web_origin = ?self.cors_web_origin,
            allow_codex_login = self.allow_codex_login,
            run_engine = ?self.run_engine,
            codex_on_path = self.codex_executable.is_some(),
            sprites_api_base = %self.sprites_api_base,
            max_concurrent_runs = self.max_concurrent_runs,
            run_timeout_secs = self.run_timeout_secs,
            default_model = DEFAULT_MODEL,
            "cloud-host configuration loaded"
        );
    }
}

fn which_codex_on_path() -> Option<PathBuf> {
    codex_provider::which_codex_executable().ok()
}

fn require_env(key: &str) -> Result<String, String> {
    env::var(key).map_err(|_| format!("{key} is required"))
}

fn parse_auth_mode(raw: &str) -> Result<AuthMode, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "internal_token" | "internal" => Ok(AuthMode::InternalToken),
        "jwt" => Ok(AuthMode::Jwt),
        "hybrid" => Ok(AuthMode::Hybrid),
        other => Err(format!("invalid ELSEWHERE_AUTH_MODE: {other}")),
    }
}
