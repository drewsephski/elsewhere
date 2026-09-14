use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use agent_core::{
    approval_action_summary, sanitize_tool_arguments, ApprovalDecision, ApprovalError, EventSink,
    ToolApprovalContext, ToolOperationKind,
};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::PgPool;
use tokio::time::{sleep, timeout};
use uuid::Uuid;

use crate::events::cloud_event_sink::CloudEventSink;
use crate::approval::registry::{ApprovalResolution, ApprovalWaitRegistry};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ToolApprovalRow {
    pub id: String,
    pub run_id: String,
    pub owner_id: String,
    pub tool_name: String,
    pub tool_kind: String,
    pub arguments_json: Value,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<String>,
    pub resolution_reason: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub requested_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct ApprovalService {
    pub pool: PgPool,
    pub registry: Arc<ApprovalWaitRegistry>,
    pub timeout: Duration,
}

impl ApprovalService {
    fn clone_shallow(&self) -> Self {
        self.clone()
    }

    pub async fn cancel_pending_for_run(&self, run_id: &str, reason: &str) -> Result<u64, sqlx::Error> {
        let pending: Vec<(String,)> = sqlx::query_as(
            "SELECT id FROM tool_approval_requests WHERE run_id = $1 AND status = 'pending'",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?;

        let mut count = 0u64;
        for (id,) in pending {
            if self
                .resolve_pending(
                    &id,
                    run_id,
                    None,
                    "cancelled",
                    None,
                    Some(reason),
                )
                .await?
            {
                count += 1;
            }
        }
        Ok(count)
    }

    pub async fn cancel_all_pending_on_host_restart(&self) -> Result<u64, sqlx::Error> {
        let pending: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, run_id FROM tool_approval_requests WHERE status = 'pending'",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut count = 0u64;
        for (id, run_id) in pending {
            if self
                .resolve_pending(
                    &id,
                    &run_id,
                    None,
                    "cancelled",
                    None,
                    Some("host_restart"),
                )
                .await?
            {
                count += 1;
            }
        }
        Ok(count)
    }

    pub async fn list_for_owner(
        &self,
        owner_id: &str,
        status: Option<&str>,
    ) -> Result<Vec<ToolApprovalRow>, sqlx::Error> {
        if let Some(status) = status {
            sqlx::query_as(
                r#"
                SELECT id, run_id, owner_id, tool_name, tool_kind, arguments_json, status,
                       created_at, updated_at, resolved_at, resolved_by, resolution_reason,
                       expires_at, requested_at
                FROM tool_approval_requests
                WHERE owner_id = $1 AND status = $2
                ORDER BY requested_at DESC
                "#,
            )
            .bind(owner_id)
            .bind(status)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as(
                r#"
                SELECT id, run_id, owner_id, tool_name, tool_kind, arguments_json, status,
                       created_at, updated_at, resolved_at, resolved_by, resolution_reason,
                       expires_at, requested_at
                FROM tool_approval_requests
                WHERE owner_id = $1
                ORDER BY requested_at DESC
                LIMIT 200
                "#,
            )
            .bind(owner_id)
            .fetch_all(&self.pool)
            .await
        }
    }

    pub async fn get_for_owner(
        &self,
        owner_id: &str,
        approval_id: &str,
    ) -> Result<Option<ToolApprovalRow>, sqlx::Error> {
        sqlx::query_as(
            r#"
            SELECT id, run_id, owner_id, tool_name, tool_kind, arguments_json, status,
                   created_at, updated_at, resolved_at, resolved_by, resolution_reason,
                   expires_at, requested_at
            FROM tool_approval_requests
            WHERE id = $1 AND owner_id = $2
            "#,
        )
        .bind(approval_id)
        .bind(owner_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn approve(
        &self,
        owner_id: &str,
        approval_id: &str,
        resolved_by: &str,
    ) -> Result<bool, sqlx::Error> {
        let row = self.get_for_owner(owner_id, approval_id).await?;
        let Some(row) = row else {
            return Ok(false);
        };
        self.resolve_pending(
            approval_id,
            &row.run_id,
            Some(resolved_by),
            "approved",
            Some("user_approved"),
            None,
        )
        .await
    }

    pub async fn deny(
        &self,
        owner_id: &str,
        approval_id: &str,
        resolved_by: &str,
        reason: Option<&str>,
    ) -> Result<bool, sqlx::Error> {
        let row = self.get_for_owner(owner_id, approval_id).await?;
        let Some(row) = row else {
            return Ok(false);
        };
        self.resolve_pending(
            approval_id,
            &row.run_id,
            Some(resolved_by),
            "denied",
            Some(reason.unwrap_or("user_denied")),
            None,
        )
        .await
    }

    async fn resolve_pending(
        &self,
        approval_id: &str,
        run_id: &str,
        resolved_by: Option<&str>,
        status: &str,
        resolution_reason: Option<&str>,
        notify_reason: Option<&str>,
    ) -> Result<bool, sqlx::Error> {
        let now = Utc::now();
        let result = sqlx::query(
            r#"
            UPDATE tool_approval_requests
            SET status = $3,
                resolved_at = $4,
                resolved_by = $5,
                resolution_reason = $6,
                updated_at = $4
            WHERE id = $1
              AND run_id = $2
              AND status = 'pending'
            "#,
        )
        .bind(approval_id)
        .bind(run_id)
        .bind(status)
        .bind(now)
        .bind(resolved_by)
        .bind(resolution_reason)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() != 1 {
            self.registry.drop_waiter(approval_id);
            return Ok(false);
        }

        let resolution = match status {
            "approved" => ApprovalResolution::Approved,
            "denied" => ApprovalResolution::Denied {
                reason: resolution_reason
                    .unwrap_or("denied")
                    .to_string(),
            },
            "cancelled" => ApprovalResolution::Cancelled {
                reason: notify_reason
                    .or(resolution_reason)
                    .unwrap_or("cancelled")
                    .to_string(),
            },
            "expired" => ApprovalResolution::Expired,
            _ => ApprovalResolution::Denied {
                reason: "invalid resolution".into(),
            },
        };
        self.registry.notify(approval_id, resolution);
        Ok(true)
    }

    pub async fn wait_for_mutation_approval(
        &self,
        context: &ToolApprovalContext,
        events: &Arc<CloudEventSink>,
        store: &Arc<dyn agent_core::RunStore>,
        cancel: &Arc<AtomicBool>,
    ) -> Result<ApprovalDecision, ApprovalError> {
        if context.operation_kind == ToolOperationKind::Read {
            return Ok(ApprovalDecision::Allow);
        }

        if cancel.load(Ordering::Relaxed) {
            return Err(ApprovalError::Cancelled);
        }

        let approval_id = Uuid::new_v4().to_string();
        let sanitized = sanitize_tool_arguments(&context.tool_name, &context.arguments);
        let summary = approval_action_summary(&context.tool_name, &sanitized);
        let expires_at = Utc::now() + chrono::Duration::from_std(self.timeout).unwrap_or(chrono::Duration::minutes(5));

        sqlx::query(
            r#"
            INSERT INTO tool_approval_requests (
                id, run_id, owner_id, tool_name, tool_kind, arguments_json,
                status, expires_at, requested_at, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, 'pending', $7, NOW(), NOW(), NOW())
            "#,
        )
        .bind(&approval_id)
        .bind(&context.run_id)
        .bind(&context.owner_id)
        .bind(&context.tool_name)
        .bind(match context.operation_kind {
            ToolOperationKind::Read => "read",
            ToolOperationKind::Mutation => "mutation",
        })
        .bind(&sanitized)
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .map_err(|e| ApprovalError::Internal(e.to_string()))?;

        let requested_payload = json!({
            "approvalId": approval_id,
            "tool": context.tool_name,
            "operationKind": "mutation",
            "summary": summary,
            "waitingForApproval": true,
        });

        let receipt = store
            .append_run_event(&context.request_id, "approval_requested", &requested_payload)
            .await
            .map_err(|e| ApprovalError::Internal(e.to_string()))?;
        let _ = events.emit_durable(receipt.id, "approval_requested", &requested_payload);

        let rx = self.registry.register(&approval_id);
        let cancel_flag = cancel.clone();
        let approval_id_wait = approval_id.clone();
        let run_id_wait = context.run_id.clone();
        let service = self.clone_shallow();

        let wait = async move {
            let mut cancel_watcher = cancel_flag.clone();
            tokio::select! {
                res = rx => res.map_err(|_| ApprovalError::Internal("approval waiter closed".into())),
                _ = async {
                    while !cancel_watcher.load(Ordering::Relaxed) {
                        sleep(Duration::from_millis(100)).await;
                    }
                } => {
                    let _ = service
                        .resolve_pending(
                            &approval_id_wait,
                            &run_id_wait,
                            None,
                            "cancelled",
                            Some("run_cancelled"),
                            Some("run_cancelled"),
                        )
                        .await;
                    Err(ApprovalError::Cancelled)
                }
            }
        };

        let resolution = match timeout(self.timeout, wait).await {
            Ok(Ok(resolution)) => resolution,
            Ok(Err(err)) => return Err(err),
            Err(_) => {
                let _ = self
                    .resolve_pending(
                        &approval_id,
                        &context.run_id,
                        None,
                        "expired",
                        Some("timeout"),
                        None,
                    )
                    .await;
                self.registry.notify(&approval_id, ApprovalResolution::Expired);
                return Err(ApprovalError::TimedOut);
            }
        };

        let decision_str = match &resolution {
            ApprovalResolution::Approved => "approved",
            ApprovalResolution::Denied { .. } => "denied",
            ApprovalResolution::Cancelled { .. } => "cancelled",
            ApprovalResolution::Expired => "expired",
        };

        let resolved_payload = json!({
            "approvalId": approval_id,
            "decision": decision_str,
            "waitingForApproval": false,
        });
        let receipt = store
            .append_run_event(&context.request_id, "approval_resolved", &resolved_payload)
            .await
            .map_err(|e| ApprovalError::Internal(e.to_string()))?;
        let _ = events.emit_durable(receipt.id, "approval_resolved", &resolved_payload);

        match resolution {
            ApprovalResolution::Approved => Ok(ApprovalDecision::Allow),
            ApprovalResolution::Denied { reason } => Err(ApprovalError::Denied { reason }),
            ApprovalResolution::Cancelled { reason } => {
                if reason == "run_cancelled" {
                    Err(ApprovalError::Cancelled)
                } else {
                    Err(ApprovalError::Denied { reason })
                }
            }
            ApprovalResolution::Expired => Err(ApprovalError::TimedOut),
        }
    }
}
