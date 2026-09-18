use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use agent_core::{
    approval_action_summary, is_policy_overridable_tool, policy_action_label,
    sanitize_tool_arguments, ApprovalDecision, ApprovalError, EventSink, ToolApprovalContext,
    ToolOperationKind, CONNECTED_APPS_EXECUTE_TOOL,
};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::PgPool;
use tokio::time::sleep;
use uuid::Uuid;

use crate::approval::registry::{ApprovalResolution, ApprovalWaitRegistry};
use crate::events::cloud_event_sink::CloudEventSink;

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
    pub bot_id: Option<String>,
    pub bot_name: Option<String>,
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

    pub async fn cancel_pending_for_run(
        &self,
        run_id: &str,
        reason: &str,
    ) -> Result<u64, sqlx::Error> {
        let pending: Vec<(String,)> = sqlx::query_as(
            "SELECT id FROM tool_approval_requests WHERE run_id = $1 AND status = 'pending'",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?;

        let mut count = 0u64;
        for (id,) in pending {
            if self
                .resolve_pending(&id, run_id, None, "cancelled", None, Some(reason))
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
                    Some("host_restart"),
                    Some("host_restart"),
                )
                .await?
            {
                let _ = self
                    .append_restart_approval_resolved_audit(&id, &run_id)
                    .await;
                count += 1;
            }
        }
        Ok(count)
    }

    async fn append_restart_approval_resolved_audit(
        &self,
        approval_id: &str,
        run_id: &str,
    ) -> Result<(), sqlx::Error> {
        let request_id: Option<(String,)> =
            sqlx::query_as("SELECT request_id FROM agent_runs WHERE id = $1 LIMIT 1")
                .bind(run_id)
                .fetch_optional(&self.pool)
                .await?;

        let Some((request_id,)) = request_id else {
            return Ok(());
        };

        let payload = json!({
            "approvalId": approval_id,
            "decision": "cancelled",
            "waitingForApproval": false,
            "resolutionReason": "host_restart",
        });
        sqlx::query(
            r#"
            INSERT INTO run_events (request_id, event_type, payload_json)
            VALUES ($1, 'approval_resolved', $2)
            "#,
        )
        .bind(&request_id)
        .bind(&payload)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_for_owner(
        &self,
        owner_id: &str,
        status: Option<&str>,
    ) -> Result<Vec<ToolApprovalRow>, sqlx::Error> {
        if let Some(status) = status {
            sqlx::query_as(
                r#"
                SELECT a.id, a.run_id, a.owner_id, a.tool_name, a.tool_kind, a.arguments_json, a.status,
                       a.created_at, a.updated_at, a.resolved_at, a.resolved_by, a.resolution_reason,
                       a.expires_at, a.requested_at, r.bot_id, b.name AS bot_name
                FROM tool_approval_requests a
                LEFT JOIN agent_runs r ON r.id = a.run_id
                LEFT JOIN bots b ON b.id = r.bot_id
                WHERE a.owner_id = $1 AND a.status = $2
                ORDER BY a.requested_at DESC
                "#,
            )
            .bind(owner_id)
            .bind(status)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as(
                r#"
                SELECT a.id, a.run_id, a.owner_id, a.tool_name, a.tool_kind, a.arguments_json, a.status,
                       a.created_at, a.updated_at, a.resolved_at, a.resolved_by, a.resolution_reason,
                       a.expires_at, a.requested_at, r.bot_id, b.name AS bot_name
                FROM tool_approval_requests a
                LEFT JOIN agent_runs r ON r.id = a.run_id
                LEFT JOIN bots b ON b.id = r.bot_id
                WHERE a.owner_id = $1
                ORDER BY a.requested_at DESC
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
            SELECT a.id, a.run_id, a.owner_id, a.tool_name, a.tool_kind, a.arguments_json, a.status,
                   a.created_at, a.updated_at, a.resolved_at, a.resolved_by, a.resolution_reason,
                   a.expires_at, a.requested_at, r.bot_id, b.name AS bot_name
            FROM tool_approval_requests a
            LEFT JOIN agent_runs r ON r.id = a.run_id
            LEFT JOIN bots b ON b.id = r.bot_id
            WHERE a.id = $1 AND a.owner_id = $2
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
        if self.expire_pending_if_due(approval_id, &row.run_id).await? {
            return Ok(false);
        }
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

    /// Atomically mark a pending approval expired when `expires_at` has passed.
    async fn expire_pending_if_due(
        &self,
        approval_id: &str,
        run_id: &str,
    ) -> Result<bool, sqlx::Error> {
        let now = Utc::now();
        let result = sqlx::query(
            r#"
            UPDATE tool_approval_requests
            SET status = 'expired',
                resolved_at = $4,
                resolved_by = NULL,
                resolution_reason = 'timeout',
                updated_at = $4
            WHERE id = $1
              AND run_id = $2
              AND status = 'pending'
              AND expires_at IS NOT NULL
              AND expires_at <= $3
            "#,
        )
        .bind(approval_id)
        .bind(run_id)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() != 1 {
            return Ok(false);
        }

        self.registry
            .notify(approval_id, ApprovalResolution::Expired);
        Ok(true)
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
                reason: resolution_reason.unwrap_or("denied").to_string(),
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

    async fn abort_visible_approval(&self, approval_id: &str, run_id: &str, reason: &str) {
        let _ = self
            .resolve_pending(
                approval_id,
                run_id,
                None,
                "cancelled",
                Some(reason),
                Some(reason),
            )
            .await;
        self.registry.drop_waiter(approval_id);
    }

    async fn finalize_mutation_approval(
        &self,
        context: &ToolApprovalContext,
        approval_id: &str,
        resolution: ApprovalResolution,
        events: &Arc<CloudEventSink>,
        store: &Arc<dyn agent_core::RunStore>,
    ) -> Result<ApprovalDecision, ApprovalError> {
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
                if reason == "run_cancelled" || reason == "host_restart" {
                    Err(ApprovalError::Cancelled)
                } else {
                    Err(ApprovalError::Denied { reason })
                }
            }
            ApprovalResolution::Expired => Err(ApprovalError::TimedOut),
        }
    }

    async fn wait_for_resolution(
        &self,
        approval_id: &str,
        run_id: &str,
        cancel: &Arc<AtomicBool>,
        rx: tokio::sync::oneshot::Receiver<ApprovalResolution>,
    ) -> ApprovalResolution {
        let service = self.clone_shallow();
        let approval_id_wait = approval_id.to_string();
        let run_id_wait = run_id.to_string();
        let cancel_flag = cancel.clone();
        let wait_timeout = self.timeout;

        let cancel_watch = tokio::spawn(async move {
            while !cancel_flag.load(Ordering::Relaxed) {
                sleep(Duration::from_millis(50)).await;
            }
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
        });

        let expire_watch = {
            let service = self.clone_shallow();
            let approval_id_wait = approval_id.to_string();
            let run_id_wait = run_id.to_string();
            tokio::spawn(async move {
                sleep(wait_timeout).await;
                let _ = service
                    .resolve_pending(
                        &approval_id_wait,
                        &run_id_wait,
                        None,
                        "expired",
                        Some("timeout"),
                        None,
                    )
                    .await;
            })
        };

        let resolution = rx.await.unwrap_or(ApprovalResolution::Denied {
            reason: "approval waiter closed".into(),
        });

        cancel_watch.abort();
        expire_watch.abort();
        resolution
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
        let expires_at = Utc::now()
            + chrono::Duration::from_std(self.timeout).unwrap_or(chrono::Duration::minutes(5));

        let rx = self.registry.register(&approval_id);

        let insert_result = sqlx::query(
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
        .await;

        if let Err(e) = insert_result {
            self.registry.drop_waiter(&approval_id);
            return Err(ApprovalError::Internal(e.to_string()));
        }

        let bot_name = self
            .bot_name_for_owner(&context.owner_id, &context.bot_id)
            .await
            .unwrap_or(None)
            .unwrap_or_else(|| "this Bot".into());
        let policy_overridable =
            is_policy_overridable_tool(&context.tool_name) || context.connected_app.is_some();
        let requested_payload = json!({
            "approvalId": approval_id,
            "tool": context.tool_name,
            "operationKind": "mutation",
            "summary": if let Some(app) = &context.connected_app {
                format!(
                    "{bot_name} wants to use {}: {}",
                    app.app_name, app.remote_tool
                )
            } else {
                summary.clone()
            },
            "waitingForApproval": true,
            "botId": context.bot_id,
            "botName": bot_name,
            "policyOverridable": policy_overridable,
            "policyActionLabel": if context.connected_app.is_some() {
                context
                    .connected_app
                    .as_ref()
                    .map(|app| format!("{} in {}", app.remote_tool, app.app_name))
                    .unwrap_or_else(|| policy_action_label(&context.tool_name).to_string())
            } else {
                policy_action_label(&context.tool_name).to_string()
            },
            "connectedAppName": context.connected_app.as_ref().map(|a| a.app_name.clone()),
            "connectedToolName": context.connected_app.as_ref().map(|a| a.remote_tool.clone()),
            "argumentSummary": sanitized
                .get("argumentSummary")
                .or_else(|| context.arguments.get("argumentSummary")),
        });

        let persist_emit = async {
            let receipt = store
                .append_run_event(
                    &context.request_id,
                    "approval_requested",
                    &requested_payload,
                )
                .await
                .map_err(|e| ApprovalError::Internal(e.to_string()))?;
            let _ = events.emit_durable(receipt.id, "approval_requested", &requested_payload);
            Ok(())
        };

        if let Err(err) = persist_emit.await {
            self.abort_visible_approval(&approval_id, &context.run_id, "request_failed")
                .await;
            return Err(err);
        }
        let origin = std::env::var("ELSEWHERE_WEB_ORIGIN")
            .ok()
            .filter(|v| !v.trim().is_empty());
        if let Err(err) = crate::channels::delivery::enqueue_owner_attention(
            &self.pool,
            &context.run_id,
            "approval",
            origin.as_deref(),
        )
        .await
        {
            tracing::warn!(
                error = %err,
                "could not enqueue channel approval notice"
            );
        }

        let resolution = self
            .wait_for_resolution(&approval_id, &context.run_id, cancel, rx)
            .await;

        self.finalize_mutation_approval(context, &approval_id, resolution, events, store)
            .await
    }

    async fn bot_name_for_owner(
        &self,
        owner_id: &str,
        bot_id: &str,
    ) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar("SELECT name FROM bots WHERE id = $1 AND owner_id = $2")
            .bind(bot_id)
            .bind(owner_id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn persist_bot_policy_and_resolve(
        &self,
        owner_id: &str,
        approval_id: &str,
        resolved_by: &str,
        decision: crate::permission_policies::PolicyDecision,
    ) -> Result<PersistPolicyResolution, sqlx::Error> {
        use crate::permission_policies::{PermissionPolicyService, PolicyDecision};

        let row = self.get_for_owner(owner_id, approval_id).await?;
        let Some(row) = row else {
            return Ok(PersistPolicyResolution::NotFound);
        };
        if row.status != "pending" {
            return Ok(PersistPolicyResolution::NotFound);
        }
        if self.expire_pending_if_due(approval_id, &row.run_id).await? {
            return Ok(PersistPolicyResolution::NotFound);
        }
        if !is_policy_overridable_tool(&row.tool_name)
            && row.tool_name != CONNECTED_APPS_EXECUTE_TOOL
        {
            return Ok(PersistPolicyResolution::NotOverridable);
        }
        let Some(bot_id) = row.bot_id.clone() else {
            return Ok(PersistPolicyResolution::NotFound);
        };

        let (scope_key, resource_scope) = if row.tool_name == CONNECTED_APPS_EXECUTE_TOOL {
            let install_id = row
                .arguments_json
                .get("installId")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let remote_tool = row
                .arguments_json
                .get("remoteTool")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if install_id.is_empty() || remote_tool.is_empty() {
                return Ok(PersistPolicyResolution::NotOverridable);
            }
            (
                format!("install:{install_id}/tool:{remote_tool}"),
                Some(json!({
                    "installId": install_id,
                    "remoteTool": remote_tool
                })),
            )
        } else {
            (String::new(), None)
        };

        let mut tx = self.pool.begin().await?;
        let now = Utc::now();
        let policies = PermissionPolicyService::new(self.pool.clone());
        policies
            .upsert_bot_decision_in_tx(
                &mut tx,
                owner_id,
                &bot_id,
                &row.tool_name,
                decision,
                now,
                &scope_key,
                resource_scope,
            )
            .await?;

        let status = match decision {
            PolicyDecision::Allow => "approved",
            PolicyDecision::Deny => "denied",
            PolicyDecision::Ask => "approved",
        };
        let resolution_reason = match decision {
            PolicyDecision::Allow => "always_allow",
            PolicyDecision::Deny => "always_deny",
            PolicyDecision::Ask => "user_approved",
        };
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
              AND owner_id = $7
              AND status = 'pending'
            "#,
        )
        .bind(approval_id)
        .bind(&row.run_id)
        .bind(status)
        .bind(now)
        .bind(resolved_by)
        .bind(resolution_reason)
        .bind(owner_id)
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() != 1 {
            tx.rollback().await?;
            self.registry.drop_waiter(approval_id);
            return Ok(PersistPolicyResolution::NotFound);
        }

        tx.commit().await?;

        let resolution = match decision {
            PolicyDecision::Deny => ApprovalResolution::Denied {
                reason: resolution_reason.to_string(),
            },
            _ => ApprovalResolution::Approved,
        };
        self.registry.notify(approval_id, resolution);
        Ok(PersistPolicyResolution::Applied {
            bot_id,
            action: row.tool_name,
            decision: decision.as_str().to_string(),
            approval_status: status.to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistPolicyResolution {
    Applied {
        bot_id: String,
        action: String,
        decision: String,
        approval_status: String,
    },
    NotFound,
    NotOverridable,
}
