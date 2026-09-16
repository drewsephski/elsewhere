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

pub async fn run(state: AppState, leadership: &mut PgConnection) -> Result<(), String> {
    state
        .dispatcher_alive
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        // Loss of the dedicated lock connection stops the process, never silently rejoins.
        tokio::time::timeout(
            Duration::from_secs(5),
            sqlx::query("SELECT 1").execute(&mut *leadership),
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
            let _ = state
                .human_interventions
                .cancel_pending_for_run(&run_id, "run_cancelled")
                .await;
        }
        if !state.draining.load(std::sync::atomic::Ordering::SeqCst) {
            crate::routines::tick(&state.pool, chrono::Utc::now())
                .await
                .map_err(|e| e.to_string())?;
            if let Err(err) =
                crate::delegation::reconcile_pending_delegation_returns(&state.pool).await
            {
                tracing::warn!(error = %err, "delegation return reconciliation failed");
            }
            if let Err(err) = crate::artifact_handoff::reconcile_artifact_handoffs(&state).await {
                tracing::warn!(error = %err, "artifact handoff reconciliation failed");
            }
        }
        dispatch_available(&state)
            .await
            .map_err(|e| e.to_string())?;
        crate::group_router::tick(&state).await;
        if let Err(err) = crate::channels::admission::tick_pending(&state).await {
            tracing::warn!(error = %err, "channel event admission tick failed");
        }
        if let Err(err) = crate::channels::delivery::recover_sending(&state.pool).await {
            tracing::warn!(error = %err, "channel delivery recovery failed");
        }
        if let Err(err) = crate::channels::delivery::tick(&state).await {
            tracing::warn!(error = %err, "channel delivery tick failed");
        }
        *state
            .runner_heartbeat
            .lock()
            .map_err(|_| "Runner heartbeat unavailable")? = Some(std::time::Instant::now());
    }
}

pub async fn dispatch_available(state: &AppState) -> Result<usize, sqlx::Error> {
    let mut started = 0;
    while !state.draining.load(std::sync::atomic::Ordering::SeqCst) {
        let Ok(permit) = state.run_semaphore.clone().try_acquire_owned() else {
            break;
        };
        let Some(input) = crate::work::claim_next(&state.pool).await? else {
            break;
        };
        crate::runner::spawn_agent_run(state.clone(), input, permit);
        started += 1;
    }
    if started > 0 {
        tracing::info!(started, "queued work dispatched");
    }
    Ok(started)
}

/// Abort and join executions before relinquishing leadership. Do not replay side effects.
pub async fn stop_executions(state: &AppState) {
    state
        .draining
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let route_active = crate::group_router::active_group_route_tasks(state);
    if route_active > 0 {
        tracing::info!(
            active_group_routes = route_active,
            "shutting down in-flight group route tasks"
        );
    }
    let mut route_tasks = std::mem::take(
        &mut *state
            .group_route_tasks
            .lock()
            .expect("group route task registry poisoned"),
    );
    route_tasks.shutdown().await;
    let mut tasks =
        std::mem::take(&mut *state.run_tasks.lock().expect("run task registry poisoned"));
    tasks.shutdown().await;
    state.codex_login_client.lock().await.take();
}

/// Includes tasks still starting and tasks collecting artifacts, unlike the live registry.
pub fn active_executions(state: &AppState) -> usize {
    state.config.max_concurrent_runs - state.run_semaphore.available_permits()
}

pub async fn drain(state: &AppState, grace: Duration) {
    state
        .draining
        .store(true, std::sync::atomic::Ordering::SeqCst);
    tracing::info!(
        active = active_executions(state),
        active_group_routes = crate::group_router::active_group_route_tasks(state),
        grace_secs = grace.as_secs(),
        "runner draining"
    );
    let completed = tokio::time::timeout(grace, async {
        loop {
            if active_executions(state) == 0
                && crate::group_router::active_group_route_tasks(state) == 0
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .is_ok();
    tracing::info!(
        completed,
        active = active_executions(state),
        active_group_routes = crate::group_router::active_group_route_tasks(state),
        "runner drain finished"
    );
}
