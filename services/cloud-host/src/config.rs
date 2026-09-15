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
    pub browser_profiles_dir: Option<PathBuf>,
    pub tool_approval_timeout_secs: u64,
    pub enforce_tool_approvals_internal: bool,
    /// When true, the legacy local owner may bypass tool approvals (local dev only).
    pub legacy_local_approval_bypass: bool,
    pub browser_enabled: bool,
    /// Base64-encoded 32-byte AES key for connector token encryption.
    pub connector_secret_key: Option<String>,
    pub github_client_id: Option<String>,
    pub github_client_secret: Option<String>,
    pub github_oauth_redirect_uri: Option<String>,
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
        let allow_codex_login = match env::var("ELSEWHERE_ALLOW_CODEX_LOGIN")
            .ok()
            .filter(|v| !v.is_empty())
        {
            Some(v) if v == "1" || v.eq_ignore_ascii_case("true") => true,
            Some(v) if v == "0" || v.eq_ignore_ascii_case("false") => false,
            Some(_) => false,
            // Local hybrid dev: enable ChatGPT device login unless explicitly turned off.
            None => auth_mode == AuthMode::Hybrid,
        };

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
        let enforce_tool_approvals_internal = parse_bool_env("ELSEWHERE_ENFORCE_TOOL_APPROVALS")
            .unwrap_or(true);
        let legacy_local_approval_bypass = parse_bool_env("ELSEWHERE_LEGACY_LOCAL_APPROVAL_BYPASS")
            .unwrap_or(false);

        let browser_enabled = match env::var("ELSEWHERE_BROWSER_ENABLED")
            .ok()
            .filter(|v| !v.is_empty())
        {
            Some(v) if v == "0" || v.eq_ignore_ascii_case("false") => false,
            Some(v) if v == "1" || v.eq_ignore_ascii_case("true") => true,
            Some(_) => true,
            None => true,
        };

        let codex_executable = env::var("CODEX_EXECUTABLE")
            .ok()
            .map(PathBuf::from)
            .or_else(|| which_codex_on_path());

        let codex_profiles_dir = env::var("ELSEWHERE_CODEX_PROFILES_DIR")
            .ok()
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                if allow_codex_login && auth_mode == AuthMode::Hybrid {
                    std::env::current_dir()
                        .ok()
                        .map(|cwd| cwd.join(".data").join("codex-profiles"))
                } else {
                    None
                }
            });

        let browser_profiles_dir = env::var("ELSEWHERE_BROWSER_PROFILES_DIR")
            .ok()
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                if browser_enabled {
                    std::env::current_dir()
                        .ok()
                        .map(|cwd| cwd.join(".data").join("browser-profiles"))
                } else {
                    None
                }
            });

        let bind_addr = resolve_bind_addr(auth_mode)?;
        validate_bind_addr(auth_mode, &bind_addr)?;

        let connector_secret_key = env::var("ELSEWHERE_CONNECTOR_SECRET_KEY")
            .ok()
            .filter(|v| !v.is_empty());
        let github_client_id = env::var("GITHUB_CLIENT_ID")
            .ok()
            .filter(|v| !v.is_empty());
        let github_client_secret = env::var("GITHUB_CLIENT_SECRET")
            .ok()
            .filter(|v| !v.is_empty());
        let github_oauth_redirect_uri = env::var("GITHUB_OAUTH_REDIRECT_URI")
            .ok()
            .filter(|v| !v.is_empty());

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
            bind_addr,
            run_engine,
            codex_executable,
            codex_profiles_dir,
            browser_profiles_dir,
            tool_approval_timeout_secs,
            enforce_tool_approvals_internal,
            legacy_local_approval_bypass,
            browser_enabled,
            connector_secret_key,
            github_client_id,
            github_client_secret,
            github_oauth_redirect_uri,
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
            codex_profiles_dir = ?self.codex_profiles_dir,
            browser_profiles_dir = ?self.browser_profiles_dir,
            run_engine = ?self.run_engine,
            codex_on_path = self.codex_executable.is_some(),
            sprites_api_base = %self.sprites_api_base,
            max_concurrent_runs = self.max_concurrent_runs,
            run_timeout_secs = self.run_timeout_secs,
            browser_enabled = self.browser_enabled,
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

fn parse_bool_env(key: &str) -> Option<bool> {
    env::var(key).ok().map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

fn resolve_bind_addr(auth_mode: AuthMode) -> Result<String, String> {
    if let Ok(bind) = env::var("ELSEWHERE_BIND") {
        if bind.trim().is_empty() {
            return Err("ELSEWHERE_BIND must not be empty".into());
        }
        return Ok(bind);
    }
    Ok(match auth_mode {
        AuthMode::Jwt => "0.0.0.0:8080".into(),
        AuthMode::InternalToken | AuthMode::Hybrid => "127.0.0.1:8080".into(),
    })
}

fn bind_host_is_loopback(bind_addr: &str) -> bool {
    let host = bind_addr
        .rsplit_once(':')
        .map(|(host, _)| host)
        .unwrap_or(bind_addr)
        .trim()
        .to_ascii_lowercase();
    host == "127.0.0.1" || host == "localhost" || host == "::1"
}

fn validate_bind_addr(auth_mode: AuthMode, bind_addr: &str) -> Result<(), String> {
    let allow_public = parse_bool_env("ELSEWHERE_ALLOW_PUBLIC_BIND").unwrap_or(false);
    if matches!(auth_mode, AuthMode::InternalToken | AuthMode::Hybrid)
        && !bind_host_is_loopback(bind_addr)
        && !allow_public
    {
        return Err(
            "Internal or hybrid auth must bind to loopback (default 127.0.0.1:8080) or set ELSEWHERE_ALLOW_PUBLIC_BIND=true".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jwt_default_bind_allows_public() {
        std::env::remove_var("ELSEWHERE_BIND");
        assert_eq!(
            resolve_bind_addr(AuthMode::Jwt).expect("bind"),
            "0.0.0.0:8080"
        );
        assert!(validate_bind_addr(AuthMode::Jwt, "0.0.0.0:8080").is_ok());
    }

    #[test]
    fn hybrid_default_bind_is_loopback() {
        std::env::remove_var("ELSEWHERE_BIND");
        assert_eq!(
            resolve_bind_addr(AuthMode::Hybrid).expect("bind"),
            "127.0.0.1:8080"
        );
        assert!(validate_bind_addr(AuthMode::Hybrid, "127.0.0.1:8080").is_ok());
    }

    #[test]
    fn hybrid_public_bind_requires_explicit_opt_in() {
        std::env::remove_var("ELSEWHERE_ALLOW_PUBLIC_BIND");
        assert!(validate_bind_addr(AuthMode::Hybrid, "0.0.0.0:8080").is_err());
        std::env::set_var("ELSEWHERE_ALLOW_PUBLIC_BIND", "true");
        assert!(validate_bind_addr(AuthMode::Hybrid, "0.0.0.0:8080").is_ok());
        std::env::remove_var("ELSEWHERE_ALLOW_PUBLIC_BIND");
    }

    #[test]
    fn approval_defaults_favor_enforcement() {
        std::env::remove_var("ELSEWHERE_ENFORCE_TOOL_APPROVALS");
        assert_eq!(parse_bool_env("ELSEWHERE_ENFORCE_TOOL_APPROVALS"), None);
        assert_eq!(
            parse_bool_env("ELSEWHERE_ENFORCE_TOOL_APPROVALS").unwrap_or(true),
            true
        );
    }
}
