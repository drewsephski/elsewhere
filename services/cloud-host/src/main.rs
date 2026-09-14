use cloud_host::{build_router, AppState, Config};
use tracing_subscriber::EnvFilter;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
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

    let leadership = cloud_host::worker::acquire_runner(&config.database_url).await?;
    let pool = sqlx::PgPool::connect(&config.database_url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

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

    let worker_state = state.clone();
    let dispatcher = tokio::spawn(async move {
        let mut leadership = leadership;
        cloud_host::worker::run(worker_state, &mut leadership).await
    });

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "Elsewhere cloud-host listening");

    let shutdown = async {
        shutdown_signal().await?;
        cloud_host::worker::drain(&state, std::time::Duration::from_secs(240)).await;
        Ok(())
    };

    let outcome: Result<(), String> = tokio::select! {
        result = axum::serve(listener, app) => result.map_err(|e| e.to_string()),
        result = shutdown => result,
        dispatcher_result = dispatcher => match dispatcher_result {
            Ok(Ok(())) => Err("runner dispatcher exited unexpectedly".into()),
            Ok(Err(err)) => {
                state
                    .draining
                    .store(true, std::sync::atomic::Ordering::SeqCst);
                Err(err)
            }
            Err(err) => {
                state
                    .draining
                    .store(true, std::sync::atomic::Ordering::SeqCst);
                Err(err.to_string())
            }
        },
    };

    cloud_host::worker::stop_executions(&state).await;
    match outcome {
        Ok(()) => Ok(()),
        Err(err) => {
            tracing::error!(error = %err, "cloud-host exiting after failure");
            std::process::exit(1);
        }
    }
}

async fn shutdown_signal() -> Result<(), String> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .map_err(|e| e.to_string())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result.map_err(|e| e.to_string()),
            _ = terminate.recv() => Ok(()),
        }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c().await.map_err(|e| e.to_string())
}
