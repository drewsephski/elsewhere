use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use cloud_host::{build_router, AppState, Config};
use tracing_subscriber::EnvFilter;

static TOKIO_HEARTBEAT_EPOCH_SECS: AtomicU64 = AtomicU64::new(0);

fn epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn runtime_stall_watchdog_enabled() -> bool {
    match std::env::var("ELSEWHERE_RUNTIME_STALL_WATCHDOG")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
    {
        Some(value) if matches!(value.as_str(), "0" | "false" | "off" | "no") => false,
        Some(value) if matches!(value.as_str(), "1" | "true" | "on" | "yes") => true,
        Some(value) => {
            eprintln!(
                "warning: ignoring invalid ELSEWHERE_RUNTIME_STALL_WATCHDOG={value:?}; expected 0/1"
            );
            !cfg!(debug_assertions)
        }
        // Local `cargo run` uses dev/debug builds; production release keeps the watchdog on.
        None => !cfg!(debug_assertions),
    }
}

fn runtime_stall_threshold_secs() -> u64 {
    std::env::var("ELSEWHERE_RUNTIME_STALL_SECS")
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .filter(|secs| *secs >= 5)
        .unwrap_or(if cfg!(debug_assertions) { 120 } else { 20 })
}

fn spawn_runtime_watchdog() {
    if !runtime_stall_watchdog_enabled() {
        tracing::info!("tokio runtime stall watchdog disabled for this process");
        return;
    }

    let stall_secs = runtime_stall_threshold_secs();
    const TICK_SECS: u64 = 1;
    const CHECK_EVERY_SECS: u64 = 3;

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(TICK_SECS));
        loop {
            interval.tick().await;
            TOKIO_HEARTBEAT_EPOCH_SECS.store(epoch_secs(), Ordering::Relaxed);
        }
    });

    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(CHECK_EVERY_SECS));
        let last = TOKIO_HEARTBEAT_EPOCH_SECS.load(Ordering::Relaxed);
        if last == 0 {
            continue;
        }
        let age = epoch_secs().saturating_sub(last);
        if age > stall_secs {
            eprintln!(
                "fatal runtime_stall: tokio heartbeat stale for {}s (threshold {}s)",
                age, stall_secs
            );
            std::process::exit(1);
        }
    });
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    spawn_runtime_watchdog();

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
        if let Err(err) = cloud_host::run_lifecycle::reconcile_collaboration_lifecycle(&pool).await
        {
            tracing::warn!(
                error = %err,
                "collaboration lifecycle reconciliation after restart failed"
            );
        }
    }
    match cloud_host::subagents::reconcile_orphaned_subagents(&pool).await {
        Ok(count) if count > 0 => {
            tracing::warn!(
                count,
                "marked orphaned subagents interrupted after host restart"
            );
        }
        Ok(_) => {}
        Err(err) => {
            tracing::warn!(
                error = %err,
                "subagent reconciliation after restart failed"
            );
        }
    }

    let state = AppState::new(pool.clone(), config.clone());
    let cancelled_approvals = state.approvals.cancel_all_pending_on_host_restart().await?;
    if cancelled_approvals > 0 {
        tracing::warn!(
            count = cancelled_approvals,
            "cancelled stale tool approvals after host restart"
        );
    }
    let cancelled_interventions = state
        .human_interventions
        .cancel_all_pending_on_host_restart()
        .await?;
    if cancelled_interventions > 0 {
        tracing::warn!(
            count = cancelled_interventions,
            "cancelled stale human interventions after host restart"
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
