use cloud_host::{build_router, AppState, Config};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = Config::from_env().map_err(|e| {
        eprintln!("configuration error: {e}");
        e
    })?;
    config.log_summary();

    let pool = sqlx::PgPool::connect(&config.database_url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    let leadership = cloud_host::worker::acquire_runner(&config.database_url).await?;

    let interrupted = cloud_host::db::queries::mark_interrupted_runs(&pool).await?;
    if interrupted > 0 {
        tracing::warn!(
            count = interrupted,
            "marked orphan runs interrupted after host restart"
        );
    }

    let state = AppState::new(pool.clone(), config.clone());
    let cancelled_approvals = state.approvals.cancel_all_pending_on_host_restart().await?;
    if cancelled_approvals > 0 {
        tracing::warn!(
            count = cancelled_approvals,
            "cancelled stale tool approvals after host restart"
        );
    }
    let app = build_router(state.clone());

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "Elsewhere cloud-host listening");
    tokio::select! {
        result = axum::serve(listener, app) => { result?; }
        result = cloud_host::worker::run(state, leadership) => { result?; }
        _ = tokio::signal::ctrl_c() => { tracing::info!("Elsewhere runner stopping; unfinished work will be marked interrupted on restart"); }
    }
    Ok(())
}
