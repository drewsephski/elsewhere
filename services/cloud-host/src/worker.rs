//! One supervised runner per deployment. A second host cannot interrupt the first host's work.
use crate::AppState;
use sqlx::{Connection, PgConnection};
use std::time::Duration;

pub async fn acquire_runner(database_url: &str) -> Result<PgConnection, String> {
    let mut connection = PgConnection::connect(database_url)
        .await
        .map_err(|e| e.to_string())?;
    let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(18493722)")
        .fetch_one(&mut connection)
        .await
        .map_err(|e| e.to_string())?;
    if !acquired {
        return Err("Another Elsewhere runner is already active for this database".into());
    }
    Ok(connection)
}

pub async fn run(state: AppState, mut leadership: PgConnection) -> Result<(), String> {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        // Loss of the dedicated lock connection stops the process, never silently rejoins.
        tokio::time::timeout(
            Duration::from_secs(5),
            sqlx::query("SELECT 1").execute(&mut leadership),
        )
        .await
        .map_err(|_| "Runner database heartbeat timed out".to_string())?
        .map_err(|e| e.to_string())?;
        let cancelled: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM agent_runs WHERE status = 'running' AND cancel_requested",
        )
        .fetch_all(&state.pool)
        .await
        .map_err(|e| e.to_string())?;
        for run_id in cancelled {
            state.registry.cancel(&run_id);
            state
                .approvals
                .cancel_pending_for_run(&run_id, "run_cancelled")
                .await
                .map_err(|e| e.to_string())?;
        }
        crate::routines::tick(&state.pool, chrono::Utc::now())
            .await
            .map_err(|e| e.to_string())?;
        dispatch_available(&state)
            .await
            .map_err(|e| e.to_string())?;
    }
}

pub async fn dispatch_available(state: &AppState) -> Result<usize, sqlx::Error> {
    let mut started = 0;
    while let Ok(permit) = state.run_semaphore.clone().try_acquire_owned() {
        let Some(input) = crate::work::claim_next(&state.pool).await? else {
            break;
        };
        crate::runner::spawn_agent_run(state.clone(), input, permit);
        started += 1;
    }
    Ok(started)
}
