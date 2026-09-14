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

    let interrupted = cloud_host::db::queries::mark_interrupted_runs(&pool).await?;
    if interrupted > 0 {
        tracing::warn!(count = interrupted, "marked orphan runs interrupted after host restart");
    }

    let state = AppState::new(pool, config.clone());
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "Elsewhere cloud-host listening");
    axum::serve(listener, app).await?;
    Ok(())
}
