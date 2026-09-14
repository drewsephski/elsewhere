use agent_core::DEFAULT_MODEL;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub openai_api_key: String,
    pub sprite_token: String,
    pub api_token: String,
    pub sprites_api_base: String,
    pub max_concurrent_runs: usize,
    pub run_timeout_secs: u64,
    pub bind_addr: String,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let database_url = require_env("DATABASE_URL")?;
        let openai_api_key = require_env("OPENAI_API_KEY")?;
        let sprite_token = env::var("SPRITE_TOKEN")
            .or_else(|_| env::var("SPRITES_TOKEN"))
            .map_err(|_| "SPRITE_TOKEN (or deprecated SPRITES_TOKEN) is required".to_string())?;
        let api_token = require_env("ELSEWHERE_CLOUD_API_TOKEN")?;

        let max_concurrent_runs = env::var("ELSEWHERE_MAX_CONCURRENT_RUNS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);
        let run_timeout_secs = env::var("ELSEWHERE_RUN_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(15 * 60);

        Ok(Self {
            database_url,
            openai_api_key,
            sprite_token,
            api_token,
            sprites_api_base: env::var("SPRITES_API_BASE")
                .unwrap_or_else(|_| sprite_computer::DEFAULT_API_BASE.to_string()),
            max_concurrent_runs,
            run_timeout_secs,
            bind_addr: env::var("ELSEWHERE_BIND").unwrap_or_else(|_| "0.0.0.0:8080".into()),
        })
    }

    pub fn log_summary(&self) {
        tracing::info!(
            database = true,
            openai = true,
            sprite = true,
            cloud_api_auth = true,
            sprites_api_base = %self.sprites_api_base,
            max_concurrent_runs = self.max_concurrent_runs,
            run_timeout_secs = self.run_timeout_secs,
            default_model = DEFAULT_MODEL,
            "cloud-host configuration loaded"
        );
    }
}

fn require_env(key: &str) -> Result<String, String> {
    env::var(key).map_err(|_| format!("{key} is required"))
}
