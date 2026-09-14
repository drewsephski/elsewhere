use crate::{auth::Principal, error::ApiError, AppState};
use axum::{extract::State, Extension, Json};
use serde::Serialize;

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct BotPresence {
    pub id: String,
    pub name: String,
    pub computer_name: Option<String>,
    pub presence: String,
    pub work_id: Option<String>,
    pub task: Option<String>,
}
#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCounts {
    pub working: i64,
    pub queued: i64,
    pub approvals: i64,
    pub finished: i64,
    pub results: i64,
    pub routines: i64,
}
#[derive(Serialize)]
pub struct WorkspaceOverview {
    pub counts: WorkspaceCounts,
    pub bots: Vec<BotPresence>,
    #[serde(rename = "runnerReady")]
    pub runner_ready: bool,
}

pub async fn overview(
    State(state): State<AppState>,
    Extension(owner): Extension<Principal>,
) -> Result<Json<WorkspaceOverview>, ApiError> {
    let counts = sqlx::query_as("SELECT (SELECT COUNT(*) FROM agent_runs WHERE owner_id=$1 AND status='running') AS working, (SELECT COUNT(*) FROM agent_runs WHERE owner_id=$1 AND status='queued') AS queued, (SELECT COUNT(*) FROM tool_approval_requests WHERE owner_id=$1 AND status='pending' AND expires_at > NOW()) AS approvals, (SELECT COUNT(*) FROM agent_runs WHERE owner_id=$1 AND status='completed') AS finished, (SELECT COUNT(*) FROM work_results f JOIN agent_runs r ON r.id=f.run_id WHERE r.owner_id=$1) AS results, (SELECT COUNT(*) FROM routines WHERE owner_id=$1 AND enabled) AS routines")
        .bind(owner.owner_id()).fetch_one(&state.pool).await.map_err(|e| ApiError::Internal(e.to_string()))?;
    let bots = sqlx::query_as(r#"
        SELECT b.id, b.name, s.display_name AS computer_name,
        CASE
          WHEN EXISTS(SELECT 1 FROM tool_approval_requests a JOIN agent_runs r ON r.id=a.run_id WHERE r.bot_id=b.id AND a.owner_id=b.owner_id AND a.status='pending' AND a.expires_at > NOW() AND r.status='running') THEN 'waiting_approval'
          WHEN current.status = 'running' THEN 'working'
          WHEN current.status = 'queued' THEN 'queued'
          WHEN current.status = 'completed' AND current.execution_released_at IS NULL AND current.started_at IS NOT NULL THEN 'saving_results'
          WHEN s.id IS NULL OR s.state = 'archived' THEN 'needs_computer'
          WHEN current.status IN ('failed','interrupted') THEN 'needs_attention'
          ELSE 'ready'
        END AS presence,
        current.id AS work_id, LEFT(q.user_message, 160) AS task
        FROM bots b
        LEFT JOIN sandboxes s ON s.id=b.computer_id AND s.owner_id=b.owner_id
        LEFT JOIN LATERAL (
          SELECT r.id, r.status, r.started_at, r.execution_released_at FROM agent_runs r WHERE r.bot_id=b.id AND r.owner_id=b.owner_id
          ORDER BY CASE WHEN r.status='running' OR (r.started_at IS NOT NULL AND r.execution_released_at IS NULL) THEN 0 WHEN r.status='queued' THEN 1 ELSE 2 END, r.created_at DESC LIMIT 1
        ) current ON TRUE
        LEFT JOIN work_queue q ON q.run_id=current.id
        WHERE b.owner_id=$1 ORDER BY b.updated_at DESC LIMIT 100
    "#).bind(owner.owner_id()).fetch_all(&state.pool).await.map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(WorkspaceOverview {
        counts,
        bots,
        runner_ready: super::health::runner_ready(&state),
    }))
}
