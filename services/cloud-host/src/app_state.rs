use std::sync::Arc;
use std::time::Duration;

use codex_provider::CodexAppServerClient;
use sqlx::PgPool;
use tokio::sync::{Mutex, Semaphore};

use crate::approval::ApprovalService;
use crate::auth::JwtVerifier;
use crate::config::Config;
use crate::events::registry::RunRegistry;

pub struct PendingCodexLogin {
    pub owner_id: String,
    pub login_id: String,
    pub auth_url: String,
    pub user_code: String,
    pub expires_at: std::time::Instant,
    pub client: Arc<CodexAppServerClient>,
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
    pub registry: Arc<RunRegistry>,
    pub run_semaphore: Arc<Semaphore>,
    pub jwt_verifier: Option<Arc<JwtVerifier>>,
    pub codex_login_client: Arc<Mutex<Option<PendingCodexLogin>>>,
    pub approvals: ApprovalService,
    pub draining: Arc<std::sync::atomic::AtomicBool>,
    pub run_tasks: Arc<std::sync::Mutex<tokio::task::JoinSet<()>>>,
    pub runner_heartbeat: Arc<std::sync::Mutex<Option<std::time::Instant>>>,
}

impl AppState {
    pub fn new(pool: PgPool, config: Config) -> Self {
        let permits = config.max_concurrent_runs;
        let jwt_verifier = match (
            config.jwt_jwks_url.as_ref(),
            config.jwt_issuer.as_ref(),
            config.jwt_audience.as_ref(),
        ) {
            (Some(jwks), Some(iss), Some(aud)) => {
                Some(Arc::new(JwtVerifier::new(crate::auth::JwtVerifierConfig {
                    jwks_url: jwks.clone(),
                    issuer: iss.clone(),
                    audience: aud.clone(),
                })))
            }
            _ => None,
        };
        let approvals = ApprovalService {
            pool: pool.clone(),
            registry: Arc::new(crate::approval::ApprovalWaitRegistry::default()),
            timeout: Duration::from_secs(config.tool_approval_timeout_secs),
        };
        Self {
            pool,
            config: Arc::new(config),
            registry: Arc::new(RunRegistry::default()),
            run_semaphore: Arc::new(Semaphore::new(permits)),
            jwt_verifier,
            codex_login_client: Arc::new(Mutex::new(None)),
            approvals,
            draining: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            run_tasks: Arc::new(std::sync::Mutex::new(tokio::task::JoinSet::new())),
            runner_heartbeat: Arc::new(std::sync::Mutex::new(None)),
        }
    }
}
