//! Shared helpers for cloud-host integration tests.

use cloud_host::Config;

pub fn test_config() -> Config {
    Config {
        database_url: std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://elsewhere:elsewhere@127.0.0.1:5432/elsewhere".into()),
        openai_api_key: Some("test-key".into()),
        sprite_token: "test-sprite".into(),
        api_token: "test-token".into(),
        auth_mode: cloud_host::config::AuthMode::InternalToken,
        jwt_issuer: None,
        jwt_audience: None,
        jwt_jwks_url: None,
        cors_web_origin: None,
        allow_codex_login: false,
        sprites_api_base: "http://127.0.0.1:9".into(),
        max_concurrent_runs: 2,
        run_timeout_secs: 120,
        bind_addr: "127.0.0.1:0".into(),
        run_engine: cloud_host::run_engine_select::RunEngineMode::Responses,
        codex_executable: None,
        codex_profiles_dir: None,
        browser_profiles_dir: None,
        tool_approval_timeout_secs: 300,
        enforce_tool_approvals_internal: false,
        legacy_local_approval_bypass: false,
        browser_enabled: false,
        connector_secret_key: None,
        github_client_id: None,
        github_client_secret: None,
        github_oauth_redirect_uri: None,
        slack_client_id: None,
        slack_client_secret: None,
        slack_signing_secret: None,
        slack_oauth_redirect_uri: None,
        slack_api_base: "https://slack.com/api".into(),
    }
}
