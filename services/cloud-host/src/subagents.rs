//! Durable ephemeral subagents for a parent cloud-host run.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use agent_core::{
    bound_subagent_result, subagent_developer_instructions, subagent_task_summary,
    subagent_user_prompt, validate_subagent_request, AgentSubagents, EventSink, RunStore,
    SubagentContext, SubagentError, SubagentRequest, SubagentResult, SubagentTurn,
    MAX_ACTIVE_SUBAGENTS_PER_PARENT, MAX_SUBAGENTS_PER_PARENT, SUBAGENT_TURN_TIMEOUT_SECS,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::PgPool;
use tokio::sync::{Mutex, Notify};
use uuid::Uuid;

const RESULT_PREVIEW_CHARS: usize = 240;

type InflightKey = (String, String);

pub struct PostgresAgentSubagents {
    pool: PgPool,
    store: Arc<dyn RunStore>,
    events: Arc<dyn EventSink>,
    cancel: Arc<AtomicBool>,
    model: String,
    turn: RwLock<Option<Arc<dyn SubagentTurn>>>,
    inflight: Mutex<HashMap<InflightKey, Arc<Notify>>>,
}

impl PostgresAgentSubagents {
    pub fn new(
        pool: PgPool,
        store: Arc<dyn RunStore>,
        events: Arc<dyn EventSink>,
        cancel: Arc<AtomicBool>,
        model: impl Into<String>,
    ) -> Arc<Self> {
        Arc::new(Self {
            pool,
            store,
            events,
            cancel,
            model: model.into(),
            turn: RwLock::new(None),
            inflight: Mutex::new(HashMap::new()),
        })
    }

    fn executor(&self) -> Result<Arc<dyn SubagentTurn>, SubagentError> {
        self.turn
            .read()
            .expect("subagent turn lock")
            .clone()
            .ok_or_else(|| {
                SubagentError::Unsupported(
                    "A subagent cannot run because this assignment has no helper executor.".into(),
                )
            })
    }

    fn cancelled(&self, ctx: &SubagentContext) -> bool {
        self.cancel.load(Ordering::Relaxed) || ctx.cancel.load(Ordering::Relaxed)
    }

    async fn emit(
        &self,
        request_id: &str,
        event_type: &str,
        payload: serde_json::Value,
    ) -> Result<(), SubagentError> {
        let receipt = self
            .store
            .append_run_event(request_id, event_type, &payload)
            .await?;
        self.events
            .emit_durable(receipt.id, event_type, &payload)
            .map_err(|err| SubagentError::Internal(err.to_string()))?;
        Ok(())
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
struct RunSubagentRow {
    id: String,
    name: String,
    task: String,
    status: String,
    result: Option<String>,
    error: Option<String>,
    started_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
}

impl RunSubagentRow {
    fn into_result(self) -> SubagentResult {
        SubagentResult {
            subagent_id: self.id,
            name: self.name,
            status: self.status,
            result: self.result,
            error: self.error,
        }
    }
}

#[async_trait]
impl AgentSubagents for PostgresAgentSubagents {
    fn attach_turn_executor(&self, executor: Arc<dyn SubagentTurn>) {
        *self.turn.write().expect("subagent turn lock") = Some(executor);
    }

    fn clear_turn_executor(&self) {
        *self.turn.write().expect("subagent turn lock") = None;
    }

    async fn run_subagent(
        &self,
        ctx: &SubagentContext,
        request: SubagentRequest,
    ) -> Result<SubagentResult, SubagentError> {
        if self.cancelled(ctx) {
            return Err(SubagentError::Cancelled);
        }
        let validated = validate_subagent_request(&request)?;
        let key = (ctx.parent_run_id.clone(), ctx.tool_invocation_id.clone());

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| SubagentError::Internal(e.to_string()))?;
        sqlx::query("SELECT id FROM agent_runs WHERE id = $1 FOR UPDATE")
            .bind(&ctx.parent_run_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| SubagentError::Internal(e.to_string()))?
            .ok_or_else(|| SubagentError::Internal("parent run not found".into()))?;

        let existing = sqlx::query_as::<_, RunSubagentRow>(
            r#"
            SELECT id, name, task, status, result, error, started_at, finished_at
            FROM run_subagents
            WHERE parent_run_id = $1 AND tool_invocation_id = $2
            "#,
        )
        .bind(&ctx.parent_run_id)
        .bind(&ctx.tool_invocation_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| SubagentError::Internal(e.to_string()))?;

        if let Some(row) = existing {
            tx.commit()
                .await
                .map_err(|e| SubagentError::Internal(e.to_string()))?;
            if is_terminal(&row.status) {
                return Ok(row.into_result());
            }
            return self.wait_for_existing(ctx, &key, &row.id).await;
        }

        let running: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM run_subagents WHERE parent_run_id = $1 AND status = 'running'",
        )
        .bind(&ctx.parent_run_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| SubagentError::Internal(e.to_string()))?;
        if running >= MAX_ACTIVE_SUBAGENTS_PER_PARENT {
            tx.commit()
                .await
                .map_err(|e| SubagentError::Internal(e.to_string()))?;
            return Err(SubagentError::LimitExceeded(
                "Only one subagent can run at a time for this assignment.".into(),
            ));
        }

        let total: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM run_subagents WHERE parent_run_id = $1")
                .bind(&ctx.parent_run_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| SubagentError::Internal(e.to_string()))?;
        if total >= MAX_SUBAGENTS_PER_PARENT {
            tx.commit()
                .await
                .map_err(|e| SubagentError::Internal(e.to_string()))?;
            return Err(SubagentError::LimitExceeded(format!(
                "This assignment already used {MAX_SUBAGENTS_PER_PARENT} subagents."
            )));
        }

        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO run_subagents (
                id, owner_id, parent_run_id, bot_id, tool_invocation_id, name, task, status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, 'running')
            "#,
        )
        .bind(&id)
        .bind(&ctx.owner_id)
        .bind(&ctx.parent_run_id)
        .bind(&ctx.bot_id)
        .bind(&ctx.tool_invocation_id)
        .bind(&validated.name)
        .bind(&validated.task)
        .execute(&mut *tx)
        .await
        .map_err(|e| SubagentError::Internal(e.to_string()))?;

        let notify = Arc::new(Notify::new());
        self.inflight
            .lock()
            .await
            .insert(key.clone(), notify.clone());
        tx.commit()
            .await
            .map_err(|e| SubagentError::Internal(e.to_string()))?;

        let task_summary = subagent_task_summary(&validated.task);
        let _ = self
            .emit(
                &ctx.parent_request_id,
                "subagent_started",
                json!({
                    "subagentId": id,
                    "name": validated.name,
                    "taskSummary": task_summary,
                    "status": "running"
                }),
            )
            .await;

        let outcome = self.execute_helper(ctx, &validated).await;
        let finished = finish_from_outcome(&id, &validated.name, outcome);
        if let Err(err) = self.persist_terminal(&id, &finished).await {
            tracing::warn!(error = %err, subagent_id = %id, "could not persist subagent terminal state");
        }
        let event_type = match finished.status.as_str() {
            "completed" => "subagent_completed",
            "cancelled" | "interrupted" => "subagent_cancelled",
            _ => "subagent_failed",
        };
        let mut payload = json!({
            "subagentId": finished.subagent_id,
            "name": finished.name,
            "taskSummary": task_summary,
            "status": finished.status,
        });
        if let Some(result) = &finished.result {
            payload["resultPreview"] = json!(preview_text(result));
        }
        if let Some(error) = &finished.error {
            payload["error"] = json!(error);
        }
        let _ = self.emit(&ctx.parent_request_id, event_type, payload).await;

        notify.notify_waiters();
        self.inflight.lock().await.remove(&key);
        if finished.status == "cancelled" {
            return Err(SubagentError::Cancelled);
        }
        Ok(finished)
    }
}

impl PostgresAgentSubagents {
    async fn execute_helper(
        &self,
        ctx: &SubagentContext,
        request: &SubagentRequest,
    ) -> Result<String, SubagentError> {
        if self.cancelled(ctx) {
            return Err(SubagentError::Cancelled);
        }
        let executor = self.executor()?;
        let model = if ctx.model.trim().is_empty() {
            self.model.clone()
        } else {
            ctx.model.clone()
        };
        let developer = subagent_developer_instructions();
        let user_prompt =
            subagent_user_prompt(&request.name, &request.task, request.context.as_deref());
        let run = executor.run_toolless(&model, &developer, &user_prompt, self.cancel.as_ref());
        match tokio::time::timeout(Duration::from_secs(SUBAGENT_TURN_TIMEOUT_SECS), run).await {
            Ok(Ok(text)) => Ok(bound_subagent_result(&text)),
            Ok(Err(err)) => Err(err),
            Err(_) => Err(SubagentError::Internal(
                "Subagent timed out before it finished.".into(),
            )),
        }
    }

    async fn persist_terminal(
        &self,
        id: &str,
        finished: &SubagentResult,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE run_subagents
            SET status = $2,
                result = $3,
                error = $4,
                finished_at = NOW()
            WHERE id = $1 AND status = 'running'
            "#,
        )
        .bind(id)
        .bind(&finished.status)
        .bind(&finished.result)
        .bind(&finished.error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn wait_for_existing(
        &self,
        ctx: &SubagentContext,
        key: &InflightKey,
        id: &str,
    ) -> Result<SubagentResult, SubagentError> {
        let notify = self.inflight.lock().await.get(key).cloned();
        let deadline =
            tokio::time::Instant::now() + Duration::from_secs(SUBAGENT_TURN_TIMEOUT_SECS);
        loop {
            if self.cancelled(ctx) {
                return Err(SubagentError::Cancelled);
            }
            if let Some(row) = load_row(&self.pool, id).await? {
                if is_terminal(&row.status) {
                    return Ok(row.into_result());
                }
            }
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(SubagentError::Internal(
                    "Timed out waiting for an in-flight subagent.".into(),
                ));
            }
            if let Some(notify) = &notify {
                let _ = tokio::time::timeout(
                    remaining.min(Duration::from_millis(200)),
                    notify.notified(),
                )
                .await;
            } else {
                tokio::time::sleep(remaining.min(Duration::from_millis(50))).await;
            }
        }
    }
}

async fn load_row(pool: &PgPool, id: &str) -> Result<Option<RunSubagentRow>, SubagentError> {
    sqlx::query_as::<_, RunSubagentRow>(
        r#"
        SELECT id, name, task, status, result, error, started_at, finished_at
        FROM run_subagents
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| SubagentError::Internal(e.to_string()))
}

fn is_terminal(status: &str) -> bool {
    matches!(status, "completed" | "failed" | "cancelled" | "interrupted")
}

fn finish_from_outcome(
    id: &str,
    name: &str,
    outcome: Result<String, SubagentError>,
) -> SubagentResult {
    match outcome {
        Ok(text) => SubagentResult {
            subagent_id: id.to_string(),
            name: name.to_string(),
            status: "completed".into(),
            result: Some(text),
            error: None,
        },
        Err(SubagentError::Cancelled) => SubagentResult {
            subagent_id: id.to_string(),
            name: name.to_string(),
            status: "cancelled".into(),
            result: None,
            error: Some("Agent run cancelled".into()),
        },
        Err(err) => SubagentResult {
            subagent_id: id.to_string(),
            name: name.to_string(),
            status: "failed".into(),
            result: None,
            error: Some(err.message()),
        },
    }
}

fn preview_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= RESULT_PREVIEW_CHARS {
        return trimmed.to_string();
    }
    let mut preview: String = trimmed.chars().take(RESULT_PREVIEW_CHARS).collect();
    preview.push('…');
    preview
}

pub async fn reconcile_orphaned_subagents(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        r#"
        UPDATE run_subagents rs
        SET status = 'interrupted',
            error = 'host_restart',
            finished_at = COALESCE(rs.finished_at, NOW())
        FROM agent_runs ar
        WHERE rs.parent_run_id = ar.id
          AND rs.status = 'running'
        RETURNING ar.request_id, rs.id, rs.name, rs.task
        "#,
    )
    .fetch_all(pool)
    .await?;

    for (request_id, id, name, task) in &rows {
        let payload = json!({
            "subagentId": id,
            "name": name,
            "taskSummary": subagent_task_summary(task),
            "status": "interrupted",
            "error": "host_restart"
        });
        let _ = sqlx::query(
            "INSERT INTO run_events (request_id, event_type, payload_json) VALUES ($1, 'subagent_cancelled', $2)",
        )
        .bind(request_id)
        .bind(&payload)
        .execute(pool)
        .await;
    }
    Ok(rows.len() as u64)
}
