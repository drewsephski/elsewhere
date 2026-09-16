use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use agent_core::{
    sanitize_human_intervention_message, validate_human_intervention_reason,
    AgentHumanIntervention, EventSink, HumanInterventionContext, HumanInterventionError,
    HumanInterventionOutcome, RunStore,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::PgPool;
use tokio::time::sleep;
use uuid::Uuid;

use crate::approval::{ApprovalResolution, ApprovalWaitRegistry};
use crate::bounded_text::truncate_utf8_bytes;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct HumanInterventionRow {
    pub id: String,
    pub run_id: String,
    pub owner_id: String,
    pub computer_id: String,
    pub reason: String,
    pub message: String,
    pub status: String,
    pub requested_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct HumanInterventionService {
    pub pool: PgPool,
    pub registry: Arc<ApprovalWaitRegistry>,
}

impl HumanInterventionService {
    pub async fn get_pending_for_owner_run(
        &self,
        owner_id: &str,
        run_id: &str,
    ) -> Result<Option<HumanInterventionRow>, sqlx::Error> {
        sqlx::query_as(
            r#"
            SELECT id, run_id, owner_id, computer_id, reason, message, status, requested_at, resolved_at
            FROM human_intervention_requests
            WHERE owner_id = $1 AND run_id = $2 AND status = 'pending'
            LIMIT 1
            "#,
        )
        .bind(owner_id)
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn cancel_pending_for_run(
        &self,
        run_id: &str,
        reason: &str,
    ) -> Result<u64, sqlx::Error> {
        let pending: Vec<(String,)> = sqlx::query_as(
            "SELECT id FROM human_intervention_requests WHERE run_id = $1 AND status = 'pending'",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?;

        let mut count = 0u64;
        for (id,) in pending {
            if self
                .resolve_pending(&id, run_id, "cancelled", Some(reason))
                .await?
            {
                count += 1;
            }
        }
        Ok(count)
    }

    pub async fn cancel_all_pending_on_host_restart(&self) -> Result<u64, sqlx::Error> {
        let pending: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, run_id FROM human_intervention_requests WHERE status = 'pending'",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut count = 0u64;
        for (id, run_id) in pending {
            if self
                .resolve_pending(&id, &run_id, "cancelled", Some("host_restart"))
                .await?
            {
                count += 1;
            }
        }
        Ok(count)
    }

    pub async fn resolve_pending_for_computer_handback(
        &self,
        owner_id: &str,
        computer_id: &str,
    ) -> Result<u64, sqlx::Error> {
        let pending: Vec<(String, String)> = sqlx::query_as(
            r#"
            SELECT id, run_id
            FROM human_intervention_requests
            WHERE owner_id = $1 AND computer_id = $2 AND status = 'pending'
            "#,
        )
        .bind(owner_id)
        .bind(computer_id)
        .fetch_all(&self.pool)
        .await?;

        let mut count = 0u64;
        for (id, run_id) in pending {
            if self
                .resolve_pending(&id, &run_id, "resolved", Some("owner_returned_control"))
                .await?
            {
                count += 1;
            }
        }
        Ok(count)
    }

    async fn resolve_pending(
        &self,
        intervention_id: &str,
        run_id: &str,
        status: &str,
        resolution_reason: Option<&str>,
    ) -> Result<bool, sqlx::Error> {
        let now = Utc::now();
        let result = sqlx::query(
            r#"
            UPDATE human_intervention_requests
            SET status = $3,
                resolved_at = $4,
                updated_at = $4
            WHERE id = $1 AND run_id = $2 AND status = 'pending'
            "#,
        )
        .bind(intervention_id)
        .bind(run_id)
        .bind(status)
        .bind(now)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() != 1 {
            self.registry.drop_waiter(intervention_id);
            return Ok(false);
        }

        let resolution = match status {
            "resolved" => ApprovalResolution::Approved,
            "cancelled" => ApprovalResolution::Cancelled {
                reason: resolution_reason.unwrap_or("cancelled").to_string(),
            },
            _ => ApprovalResolution::Denied {
                reason: "invalid resolution".into(),
            },
        };
        self.registry.notify(intervention_id, resolution);
        Ok(true)
    }

    async fn emit_requested_event(
        &self,
        request_id: &str,
        intervention_id: &str,
        computer_id: &str,
        reason: &str,
        message: &str,
        store: &Arc<dyn RunStore>,
        events: &Arc<dyn EventSink>,
    ) -> Result<(), HumanInterventionError> {
        let payload = json!({
            "interventionId": intervention_id,
            "reason": reason,
            "message": message,
            "computerId": computer_id,
            "waitingForHuman": true,
        });
        let receipt = store
            .append_run_event(request_id, "human_intervention_requested", &payload)
            .await
            .map_err(|e| HumanInterventionError::Internal(e.to_string()))?;
        events
            .emit_durable(receipt.id, "human_intervention_requested", &payload)
            .map_err(|e| HumanInterventionError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn emit_resolved_event(
        &self,
        request_id: &str,
        intervention_id: &str,
        store: &Arc<dyn RunStore>,
        events: &Arc<dyn EventSink>,
    ) -> Result<(), sqlx::Error> {
        let payload = json!({
            "interventionId": intervention_id,
            "status": "resolved",
            "waitingForHuman": false,
        });
        let receipt = store
            .append_run_event(request_id, "human_intervention_resolved", &payload)
            .await
            .map_err(|e| sqlx::Error::Protocol(format!("{e}")))?;
        let _ = events.emit_durable(receipt.id, "human_intervention_resolved", &payload);
        Ok(())
    }

    pub async fn request_and_wait(
        &self,
        ctx: &HumanInterventionContext,
        reason: &str,
        message: &str,
        cancel: Arc<AtomicBool>,
        store: Arc<dyn RunStore>,
        events: Arc<dyn EventSink>,
    ) -> Result<HumanInterventionOutcome, HumanInterventionError> {
        validate_human_intervention_reason(reason)?;
        let safe_message = sanitize_human_intervention_message(message)?;
        let bounded_message = truncate_utf8_bytes(
            &safe_message,
            agent_core::MAX_HUMAN_INTERVENTION_MESSAGE_CHARS,
        );

        if cancel.load(Ordering::Relaxed) {
            return Err(HumanInterventionError::Cancelled);
        }

        let intervention_id = Uuid::new_v4().to_string();
        let rx = self.registry.register(&intervention_id);

        let insert = sqlx::query(
            r#"
            INSERT INTO human_intervention_requests (
                id, run_id, owner_id, computer_id, reason, message, status, requested_at, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, 'pending', NOW(), NOW(), NOW())
            "#,
        )
        .bind(&intervention_id)
        .bind(&ctx.run_id)
        .bind(&ctx.owner_id)
        .bind(&ctx.computer_id)
        .bind(reason)
        .bind(&bounded_message)
        .execute(&self.pool)
        .await;

        if let Err(err) = insert {
            self.registry.drop_waiter(&intervention_id);
            if let Some(db_err) = err.as_database_error() {
                if db_err.code().as_deref() == Some("23505") {
                    return Err(HumanInterventionError::DuplicatePending);
                }
            }
            return Err(HumanInterventionError::Internal(err.to_string()));
        }

        if let Err(err) = self
            .emit_requested_event(
                &ctx.request_id,
                &intervention_id,
                &ctx.computer_id,
                reason,
                &bounded_message,
                &store,
                &events,
            )
            .await
        {
            let _ = self
                .resolve_pending(
                    &intervention_id,
                    &ctx.run_id,
                    "cancelled",
                    Some("request_failed"),
                )
                .await;
            return Err(err);
        }
        let origin = std::env::var("ELSEWHERE_WEB_ORIGIN")
            .ok()
            .filter(|v| !v.trim().is_empty());
        if let Err(err) = crate::channels::delivery::enqueue_owner_attention(
            &self.pool,
            &ctx.run_id,
            "help",
            origin.as_deref(),
        )
        .await
        {
            tracing::warn!(error = %err, "could not enqueue channel human-intervention notice");
        }

        let resolution = self
            .wait_for_resolution(&intervention_id, &ctx.run_id, cancel, rx)
            .await;

        match resolution {
            ApprovalResolution::Approved => {
                if let Err(err) = self
                    .emit_resolved_event(&ctx.request_id, &intervention_id, &store, &events)
                    .await
                {
                    tracing::warn!(error = %err, "could not emit human_intervention_resolved");
                }
                Ok(HumanInterventionOutcome { intervention_id })
            }
            ApprovalResolution::Cancelled { reason } => {
                if reason == "run_cancelled" || reason == "host_restart" {
                    Err(HumanInterventionError::Cancelled)
                } else {
                    Err(HumanInterventionError::Internal(reason))
                }
            }
            ApprovalResolution::Denied { reason } => Err(HumanInterventionError::Internal(reason)),
            ApprovalResolution::Expired => Err(HumanInterventionError::Internal(
                "intervention expired".into(),
            )),
        }
    }

    async fn wait_for_resolution(
        &self,
        intervention_id: &str,
        run_id: &str,
        cancel: Arc<AtomicBool>,
        rx: tokio::sync::oneshot::Receiver<ApprovalResolution>,
    ) -> ApprovalResolution {
        let service = self.clone();
        let intervention_id_wait = intervention_id.to_string();
        let run_id_wait = run_id.to_string();
        let cancel_flag = cancel.clone();

        let cancel_watch = tokio::spawn(async move {
            while !cancel_flag.load(Ordering::Relaxed) {
                sleep(Duration::from_millis(50)).await;
            }
            let _ = service
                .resolve_pending(
                    &intervention_id_wait,
                    &run_id_wait,
                    "cancelled",
                    Some("run_cancelled"),
                )
                .await;
        });

        let resolution = rx.await.unwrap_or(ApprovalResolution::Denied {
            reason: "intervention waiter closed".into(),
        });

        cancel_watch.abort();
        resolution
    }
}

pub struct RunScopedHumanIntervention {
    service: HumanInterventionService,
    store: Arc<dyn RunStore>,
    events: Arc<dyn EventSink>,
    cancel: Arc<AtomicBool>,
}

impl RunScopedHumanIntervention {
    pub fn new(
        service: HumanInterventionService,
        store: Arc<dyn RunStore>,
        events: Arc<dyn EventSink>,
        cancel: Arc<AtomicBool>,
    ) -> Arc<dyn AgentHumanIntervention> {
        Arc::new(Self {
            service,
            store,
            events,
            cancel,
        })
    }
}

#[async_trait]
impl AgentHumanIntervention for RunScopedHumanIntervention {
    async fn request_and_wait(
        &self,
        ctx: &HumanInterventionContext,
        reason: &str,
        message: &str,
        cancel: &AtomicBool,
    ) -> Result<HumanInterventionOutcome, HumanInterventionError> {
        self.service
            .request_and_wait(
                ctx,
                reason,
                message,
                self.cancel.clone(),
                self.store.clone(),
                self.events.clone(),
            )
            .await
    }
}
