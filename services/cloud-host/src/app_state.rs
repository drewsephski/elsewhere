use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::Semaphore;

use crate::config::Config;
use crate::events::registry::RunRegistry;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
    pub registry: Arc<RunRegistry>,
    pub run_semaphore: Arc<Semaphore>,
}

impl AppState {
    pub fn new(pool: PgPool, config: Config) -> Self {
        let permits = config.max_concurrent_runs;
        Self {
            pool,
            config: Arc::new(config),
            registry: Arc::new(RunRegistry::default()),
            run_semaphore: Arc::new(Semaphore::new(permits)),
        }
    }
}
