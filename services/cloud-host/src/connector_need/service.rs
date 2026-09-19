use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use agent_core::{EventSink, RunStore};
use chrono::Utc;
use serde_json::json;
use sqlx::PgPool;
use tokio::time::sleep;
use uuid::Uuid;

use crate::connector_need::registry::ConnectorNeedWaitRegistry;
use crate::connectors::github_access::{
    classify_github_access, resolution_for_reason, ConnectorNeedReason, ConnectorNeedRequest,
    ConnectorNeedResolution, GithubAccess,
};
use crate::connectors::service::PostgresAgentConnectors;
use crate::error::ApiError;
use crate::events::cloud_event_sink::CloudEventSink;

const PROVIDER_GITHUB: &str = "github";

#[derive(Debug, Clone, thiserror::Error)]
pub enum ConnectorNeedError {
    #[error("{0}")]
    Internal(String),
}

impl From<sqlx::Error> for ConnectorNeedError {
    fn from(value: sqlx::Error) -> Self {
        ConnectorNeedError::Internal(value.to_string())
    }
}

impl From<ApiError> for ConnectorNeedError {
    fn from(value: ApiError) -> Self {
        ConnectorNeedError::Internal(value.to_string())
    }
}

#[derive(Clone)]
pub struct ConnectorNeedService {
    pub pool: PgPool,
    pub registry: Arc<ConnectorNeedWaitRegistry>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct ConnectorNeedRow {
    id: String,
    owner_id: String,
    run_id: String,
    bot_id: String,
    tool_name: String,
    reason_kind: String,
    repo_owner: Option<String>,
    repo_name: Option<String>,
    status: String,
    created_at: chrono::DateTime<Utc>,
}

impl ConnectorNeedService {
    pub async fn request_and_wait(
        &self,
        request: ConnectorNeedRequest,
        cancel: &Arc<AtomicBool>,
        events: &Arc<CloudEventSink>,
        store: &Arc<dyn RunStore>,
        connectors: &PostgresAgentConnectors,
    ) -> Result<ConnectorNeedResolution, ConnectorNeedError> {
        if cancel.load(Ordering::Relaxed) {
            return Ok(ConnectorNeedResolution::Cancelled);
        }

        let (need_id, created_at, inserted) = self.insert_or_join(&request).await?;
        let handle = self.registry.subscribe(&need_id);

        if inserted {
            if let Err(err) = self
                .emit_needed(&request, &need_id, created_at, store, events)
                .await
            {
                let _ = self
                    .resolve_pending(&need_id, ConnectorNeedResolution::Cancelled)
                    .await;
                return Err(err);
            }
        }

        let deadline = tokio::time::Instant::now() + self.timeout;
        loop {
            if let Some(resolution) = handle.resolved() {
                return Ok(resolution);
            }
            if cancel.load(Ordering::Relaxed) {
                let _ = self
                    .resolve_pending(&need_id, ConnectorNeedResolution::Cancelled)
                    .await;
                self.emit_resolved(
                    &request.run_id,
                    &need_id,
                    ConnectorNeedResolution::Cancelled,
                    store,
                    events,
                )
                .await?;
                return Ok(ConnectorNeedResolution::Cancelled);
            }

            connectors.invalidate_github_catalog(&request.owner_id);
            match classify_github_access(
                Some(connectors),
                true,
                &request.owner_id,
                &request.tool_name,
                &request.arguments,
            )
            .await
            {
                Ok(GithubAccess::Ready) => {
                    let resolution = resolution_for_reason(&request.reason);
                    if self.resolve_pending(&need_id, resolution).await? {
                        self.emit_resolved(&request.run_id, &need_id, resolution, store, events)
                            .await?;
                    }
                    return Ok(handle.resolved().unwrap_or(resolution));
                }
                Ok(GithubAccess::Need(_)) => {}
                Err(err) => {
                    return Err(ConnectorNeedError::Internal(err.message()));
                }
            }

            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                if self
                    .resolve_pending(&need_id, ConnectorNeedResolution::Expired)
                    .await?
                {
                    self.emit_resolved(
                        &request.run_id,
                        &need_id,
                        ConnectorNeedResolution::Expired,
                        store,
                        events,
                    )
                    .await?;
                }
                return Ok(handle
                    .resolved()
                    .unwrap_or(ConnectorNeedResolution::Expired));
            }

            tokio::select! {
                _ = handle.notified() => {}
                _ = sleep(remaining) => {
                    if self
                        .resolve_pending(&need_id, ConnectorNeedResolution::Expired)
                        .await?
                    {
                        self.emit_resolved(
                            &request.run_id,
                            &need_id,
                            ConnectorNeedResolution::Expired,
                            store,
                            events,
                        )
                        .await?;
                    }
                    return Ok(handle
                        .resolved()
                        .unwrap_or(ConnectorNeedResolution::Expired));
                }
                _ = wait_for_cancel(cancel) => {
                    if self
                        .resolve_pending(&need_id, ConnectorNeedResolution::Cancelled)
                        .await?
                    {
                        self.emit_resolved(
                            &request.run_id,
                            &need_id,
                            ConnectorNeedResolution::Cancelled,
                            store,
                            events,
                        )
                        .await?;
                    }
                    return Ok(handle
                        .resolved()
                        .unwrap_or(ConnectorNeedResolution::Cancelled));
                }
            }
        }
    }

    pub async fn notify_owner_github_changed(&self, owner_id: &str) -> Result<(), ApiError> {
        let ids: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT id
            FROM connector_need_requests
            WHERE owner_id = $1 AND provider = $2 AND status = 'pending'
            "#,
        )
        .bind(owner_id)
        .bind(PROVIDER_GITHUB)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
        for (id,) in ids {
            self.registry.notify_recheck(&id);
        }
        Ok(())
    }

    async fn insert_or_join(
        &self,
        request: &ConnectorNeedRequest,
    ) -> Result<(String, chrono::DateTime<Utc>, bool), ConnectorNeedError> {
        let existing: Option<ConnectorNeedRow> = sqlx::query_as(
            r#"
            SELECT id, owner_id, run_id, bot_id, tool_name, reason_kind, repo_owner, repo_name,
                   status, created_at
            FROM connector_need_requests
            WHERE run_id = $1 AND provider = $2 AND status = 'pending'
            "#,
        )
        .bind(&request.run_id)
        .bind(PROVIDER_GITHUB)
        .fetch_optional(&self.pool)
        .await?;
        if let Some(row) = existing {
            return Ok((row.id, row.created_at, false));
        }

        let need_id = format!("cneed_{}", Uuid::new_v4());
        let created_at = Utc::now();
        let (repo_owner, repo_name) = match &request.reason {
            ConnectorNeedReason::UnauthorizedRepo { owner, repo } => {
                (Some(owner.as_str()), Some(repo.as_str()))
            }
            _ => (None, None),
        };
        let insert = sqlx::query(
            r#"
            INSERT INTO connector_need_requests (
                id, owner_id, run_id, bot_id, provider, tool_name, reason_kind,
                repo_owner, repo_name, status, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'pending', $10)
            ON CONFLICT (run_id, provider) WHERE status = 'pending'
            DO NOTHING
            "#,
        )
        .bind(&need_id)
        .bind(&request.owner_id)
        .bind(&request.run_id)
        .bind(&request.bot_id)
        .bind(PROVIDER_GITHUB)
        .bind(&request.tool_name)
        .bind(request.reason.kind_name())
        .bind(repo_owner)
        .bind(repo_name)
        .bind(created_at)
        .execute(&self.pool)
        .await?;

        if insert.rows_affected() == 1 {
            return Ok((need_id, created_at, true));
        }

        let joined: ConnectorNeedRow = sqlx::query_as(
            r#"
            SELECT id, owner_id, run_id, bot_id, tool_name, reason_kind, repo_owner, repo_name,
                   status, created_at
            FROM connector_need_requests
            WHERE run_id = $1 AND provider = $2 AND status = 'pending'
            "#,
        )
        .bind(&request.run_id)
        .bind(PROVIDER_GITHUB)
        .fetch_one(&self.pool)
        .await?;
        Ok((joined.id, joined.created_at, false))
    }

    pub async fn announce(
        &self,
        request: ConnectorNeedRequest,
        events: &Arc<CloudEventSink>,
        store: &Arc<dyn RunStore>,
    ) -> Result<(), ConnectorNeedError> {
        let (need_id, created_at, inserted) = self.insert_or_join(&request).await?;
        if inserted {
            self.emit_needed(&request, &need_id, created_at, store, events)
                .await?;
        }
        Ok(())
    }

    async fn emit_needed(
        &self,
        request: &ConnectorNeedRequest,
        need_id: &str,
        created_at: chrono::DateTime<Utc>,
        store: &Arc<dyn RunStore>,
        events: &Arc<CloudEventSink>,
    ) -> Result<(), ConnectorNeedError> {
        let request_id = self.request_id_for_run(&request.run_id).await?;
        let payload = json!({
            "needId": need_id,
            "provider": PROVIDER_GITHUB,
            "reason": request.reason,
            "runId": request.run_id,
            "botId": request.bot_id,
            "toolName": request.tool_name,
            "requestedAt": created_at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        });
        let receipt = store
            .append_run_event(&request_id, "connector_needed", &payload)
            .await
            .map_err(|e| ConnectorNeedError::Internal(e.to_string()))?;
        let _ = events.emit_durable(receipt.id, "connector_needed", &payload);
        Ok(())
    }

    async fn emit_resolved(
        &self,
        run_id: &str,
        need_id: &str,
        resolution: ConnectorNeedResolution,
        store: &Arc<dyn RunStore>,
        events: &Arc<CloudEventSink>,
    ) -> Result<(), ConnectorNeedError> {
        let request_id = self.request_id_for_run(run_id).await?;
        let payload = json!({
            "needId": need_id,
            "resolution": resolution.as_str(),
        });
        let receipt = store
            .append_run_event(&request_id, "connector_needed_resolved", &payload)
            .await
            .map_err(|e| ConnectorNeedError::Internal(e.to_string()))?;
        let _ = events.emit_durable(receipt.id, "connector_needed_resolved", &payload);
        Ok(())
    }

    async fn resolve_pending(
        &self,
        need_id: &str,
        resolution: ConnectorNeedResolution,
    ) -> Result<bool, ConnectorNeedError> {
        let now = Utc::now();
        let result = sqlx::query(
            r#"
            UPDATE connector_need_requests
            SET status = 'resolved',
                resolution = $2,
                resolved_at = $3
            WHERE id = $1 AND status = 'pending'
            "#,
        )
        .bind(need_id)
        .bind(resolution.as_str())
        .bind(now)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() != 1 {
            self.registry.drop_waiter(need_id);
            return Ok(false);
        }
        self.registry.resolve(need_id, resolution);
        Ok(true)
    }

    async fn request_id_for_run(&self, run_id: &str) -> Result<String, ConnectorNeedError> {
        sqlx::query_scalar("SELECT request_id FROM agent_runs WHERE id = $1")
            .bind(run_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| ConnectorNeedError::Internal("run is missing a request id".into()))
    }
}

async fn wait_for_cancel(cancel: &Arc<AtomicBool>) {
    while !cancel.load(Ordering::Relaxed) {
        sleep(Duration::from_millis(50)).await;
    }
}
