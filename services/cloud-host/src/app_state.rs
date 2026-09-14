use std::sync::Arc;

use codex_provider::CodexAppServerClient;
use sqlx::PgPool;
use tokio::sync::{Mutex, Semaphore};

use crate::auth::JwtVerifier;
use crate::config::Config;
use crate::events::registry::RunRegistry;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
    pub registry: Arc<RunRegistry>,
    pub run_semaphore: Arc<Semaphore>,
    pub jwt_verifier: Option<Arc<JwtVerifier>>,
    pub codex_login_client: Arc<Mutex<Option<Arc<CodexAppServerClient>>>>,
}

impl AppState {
    pub fn new(pool: PgPool, config: Config) -> Self {
        let permits = config.max_concurrent_runs;
        let jwt_verifier = match (
            config.jwt_jwks_url.as_ref(),
            config.jwt_issuer.as_ref(),
            config.jwt_audience.as_ref(),
        ) {
            (Some(jwks), Some(iss), Some(aud)) => Some(Arc::new(JwtVerifier::new(
                crate::auth::JwtVerifierConfig {
                    jwks_url: jwks.clone(),
                    issuer: iss.clone(),
                    audience: aud.clone(),
                },
            ))),
            _ => None,
        };
        Self {
            pool,
            config: Arc::new(config),
            registry: Arc::new(RunRegistry::default()),
            run_semaphore: Arc::new(Semaphore::new(permits)),
            jwt_verifier,
            codex_login_client: Arc::new(Mutex::new(None)),
        }
    }
}
