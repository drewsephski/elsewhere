use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use agent_core::{
    AgentUserQuestion, EventSink, RunStore, UserQuestionContext, UserQuestionError,
    UserQuestionOutcome, UserQuestionRequest, MAX_ASK_USER_PER_RUN,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::PgPool;
use tokio::time::sleep;
use uuid::Uuid;

use crate::approval::{ApprovalResolution, ApprovalWaitRegistry};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserQuestionRow {
    pub id: String,
    pub owner_id: String,
    pub run_id: String,
    pub request_id: String,
    pub tool_invocation_id: String,
    pub question: String,
    pub options: serde_json::Value,
    pub status: String,
    pub selected_index: Option<i32>,
    pub selected_option: Option<String>,
    pub requested_at: DateTime<Utc>,
    pub answered_at: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct UserQuestionService {
    pub pool: PgPool,
    pub registry: Arc<ApprovalWaitRegistry>,
}

impl UserQuestionService {
    pub async fn get_pending_for_owner_run(
        &self,
        owner_id: &str,
        run_id: &str,
    ) -> Result<Option<UserQuestionRow>, sqlx::Error> {
        sqlx::query_as(
            r#"
            SELECT id, owner_id, run_id, request_id, tool_invocation_id, question, options, status,
                   selected_index, selected_option, requested_at, answered_at
            FROM run_user_questions
            WHERE owner_id = $1 AND run_id = $2 AND status = 'pending'
            LIMIT 1
            "#,
        )
        .bind(owner_id)
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn get_for_owner(
        &self,
        owner_id: &str,
        run_id: &str,
        question_id: &str,
    ) -> Result<Option<UserQuestionRow>, sqlx::Error> {
        sqlx::query_as(
            r#"
            SELECT id, owner_id, run_id, request_id, tool_invocation_id, question, options, status,
                   selected_index, selected_option, requested_at, answered_at
            FROM run_user_questions
            WHERE id = $1 AND owner_id = $2 AND run_id = $3
            "#,
        )
        .bind(question_id)
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
            "SELECT id FROM run_user_questions WHERE run_id = $1 AND status = 'pending'",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?;
        let mut count = 0u64;
        for (id,) in pending {
            if self
                .resolve_pending(&id, run_id, "cancelled", Some(reason), None, None)
                .await?
            {
                count += 1;
            }
        }
        Ok(count)
    }

    pub async fn interrupt_all_pending_on_host_restart(&self) -> Result<u64, sqlx::Error> {
        let pending: Vec<(String, String)> =
            sqlx::query_as("SELECT id, run_id FROM run_user_questions WHERE status = 'pending'")
                .fetch_all(&self.pool)
                .await?;
        let mut count = 0u64;
        for (id, run_id) in pending {
            if self
                .resolve_pending(
                    &id,
                    &run_id,
                    "interrupted",
                    Some("host_restart"),
                    None,
                    None,
                )
                .await?
            {
                count += 1;
            }
        }
        Ok(count)
    }

    pub async fn answer(
        &self,
        owner_id: &str,
        run_id: &str,
        question_id: &str,
        selected_index: usize,
    ) -> Result<UserQuestionRow, crate::error::ApiError> {
        let row = self
            .get_for_owner(owner_id, run_id, question_id)
            .await
            .map_err(|e| crate::error::ApiError::Internal(e.to_string()))?
            .ok_or(crate::error::ApiError::NotFound)?;
        if row.status != "pending" {
            return Ok(row);
        }
        let options = parse_options(&row.options)?;
        if selected_index >= options.len() {
            return Err(crate::error::ApiError::Validation(
                "selectedIndex is out of range".into(),
            ));
        }
        let selected_option = options[selected_index].clone();
        let updated = sqlx::query_as(
            r#"
            UPDATE run_user_questions
            SET status = 'answered',
                selected_index = $3,
                selected_option = $4,
                answered_at = NOW(),
                updated_at = NOW()
            WHERE id = $1 AND run_id = $2 AND status = 'pending'
            RETURNING id, owner_id, run_id, request_id, tool_invocation_id, question, options, status,
                      selected_index, selected_option, requested_at, answered_at
            "#,
        )
        .bind(question_id)
        .bind(run_id)
        .bind(selected_index as i32)
        .bind(&selected_option)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| crate::error::ApiError::Internal(e.to_string()))?;
        let Some(updated) = updated else {
            return self
                .get_for_owner(owner_id, run_id, question_id)
                .await
                .map_err(|e| crate::error::ApiError::Internal(e.to_string()))?
                .ok_or(crate::error::ApiError::NotFound);
        };
        self.registry
            .notify(question_id, ApprovalResolution::Approved);
        Ok(updated)
    }

    async fn resolve_pending(
        &self,
        question_id: &str,
        run_id: &str,
        status: &str,
        reason: Option<&str>,
        selected_index: Option<i32>,
        selected_option: Option<&str>,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE run_user_questions
            SET status = $3,
                selected_index = COALESCE($4, selected_index),
                selected_option = COALESCE($5, selected_option),
                answered_at = CASE WHEN $3 = 'answered' THEN NOW() ELSE answered_at END,
                updated_at = NOW()
            WHERE id = $1 AND run_id = $2 AND status = 'pending'
            "#,
        )
        .bind(question_id)
        .bind(run_id)
        .bind(status)
        .bind(selected_index)
        .bind(selected_option)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() != 1 {
            self.registry.drop_waiter(question_id);
            return Ok(false);
        }
        let resolution = match status {
            "answered" => ApprovalResolution::Approved,
            "interrupted" => ApprovalResolution::Cancelled {
                reason: reason.unwrap_or("interrupted").to_string(),
            },
            _ => ApprovalResolution::Cancelled {
                reason: reason.unwrap_or("cancelled").to_string(),
            },
        };
        self.registry.notify(question_id, resolution);
        Ok(true)
    }

    pub async fn ask_and_wait(
        &self,
        ctx: &UserQuestionContext,
        request: &UserQuestionRequest,
        cancel: Arc<AtomicBool>,
        store: Arc<dyn RunStore>,
        events: Arc<dyn EventSink>,
    ) -> Result<UserQuestionOutcome, UserQuestionError> {
        if cancel.load(Ordering::Relaxed) {
            return Err(UserQuestionError::Cancelled);
        }
        let existing = sqlx::query_as::<_, UserQuestionRow>(
            r#"
            SELECT id, owner_id, run_id, request_id, tool_invocation_id, question, options, status,
                   selected_index, selected_option, requested_at, answered_at
            FROM run_user_questions
            WHERE run_id = $1 AND tool_invocation_id = $2
            "#,
        )
        .bind(&ctx.run_id)
        .bind(&ctx.tool_invocation_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| UserQuestionError::Internal(e.to_string()))?;
        if let Some(existing) = existing {
            return replay_existing(existing);
        }

        let used: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM run_user_questions WHERE run_id = $1")
                .bind(&ctx.run_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| UserQuestionError::Internal(e.to_string()))?;
        if used as usize >= MAX_ASK_USER_PER_RUN {
            return Err(UserQuestionError::LimitReached);
        }

        let question_id = Uuid::new_v4().to_string();
        let rx = self.registry.register(&question_id);
        let options_json = serde_json::to_value(&request.options)
            .map_err(|e| UserQuestionError::Internal(e.to_string()))?;
        let insert = sqlx::query(
            r#"
            INSERT INTO run_user_questions (
                id, owner_id, run_id, request_id, tool_invocation_id, question, options, status
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,'pending')
            "#,
        )
        .bind(&question_id)
        .bind(&ctx.owner_id)
        .bind(&ctx.run_id)
        .bind(&ctx.request_id)
        .bind(&ctx.tool_invocation_id)
        .bind(&request.question)
        .bind(&options_json)
        .execute(&self.pool)
        .await;

        if let Err(err) = insert {
            self.registry.drop_waiter(&question_id);
            if let Some(db_err) = err.as_database_error() {
                if db_err.code().as_deref() == Some("23505") {
                    let existing = sqlx::query_as::<_, UserQuestionRow>(
                        r#"
                        SELECT id, owner_id, run_id, request_id, tool_invocation_id, question, options, status,
                               selected_index, selected_option, requested_at, answered_at
                        FROM run_user_questions
                        WHERE run_id = $1 AND (tool_invocation_id = $2 OR status = 'pending')
                        ORDER BY requested_at ASC
                        LIMIT 1
                        "#,
                    )
                    .bind(&ctx.run_id)
                    .bind(&ctx.tool_invocation_id)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(|e| UserQuestionError::Internal(e.to_string()))?;
                    if let Some(existing) = existing {
                        if existing.tool_invocation_id == ctx.tool_invocation_id {
                            return replay_existing(existing);
                        }
                        return Err(UserQuestionError::DuplicatePending);
                    }
                    return Err(UserQuestionError::DuplicatePending);
                }
            }
            return Err(UserQuestionError::Internal(err.to_string()));
        }

        let requested_payload = json!({
            "questionId": question_id,
            "question": request.question,
            "options": request.options,
            "waitingForUser": true,
        });
        match store
            .append_run_event(
                &ctx.request_id,
                "user_question_requested",
                &requested_payload,
            )
            .await
        {
            Ok(receipt) => {
                let _ =
                    events.emit_durable(receipt.id, "user_question_requested", &requested_payload);
            }
            Err(err) => {
                let _ = self
                    .resolve_pending(
                        &question_id,
                        &ctx.run_id,
                        "cancelled",
                        Some("request_failed"),
                        None,
                        None,
                    )
                    .await;
                return Err(UserQuestionError::Internal(err.to_string()));
            }
        }

        let origin = std::env::var("ELSEWHERE_WEB_ORIGIN")
            .ok()
            .filter(|v| !v.trim().is_empty());
        let option_labels = request.options.join(" / ");
        let detail = format!("{}\n{}", request.question, option_labels);
        if let Err(err) = crate::channels::delivery::enqueue_owner_attention_with_detail(
            &self.pool,
            &ctx.run_id,
            "choice",
            Some(detail.as_str()),
            origin.as_deref(),
        )
        .await
        {
            tracing::warn!(error = %err, "could not enqueue channel ask_user notice");
        }

        let resolution = self
            .wait_for_resolution(&question_id, &ctx.run_id, cancel, rx)
            .await;
        match resolution {
            ApprovalResolution::Approved => {
                let row = self
                    .get_for_owner(&ctx.owner_id, &ctx.run_id, &question_id)
                    .await
                    .map_err(|e| UserQuestionError::Internal(e.to_string()))?
                    .ok_or_else(|| {
                        UserQuestionError::Internal("answered question missing".into())
                    })?;
                let answered_payload = json!({
                    "questionId": question_id,
                    "selectedIndex": row.selected_index,
                    "selectedOption": row.selected_option,
                    "waitingForUser": false,
                });
                if let Ok(receipt) = store
                    .append_run_event(&ctx.request_id, "user_question_answered", &answered_payload)
                    .await
                {
                    let _ = events.emit_durable(
                        receipt.id,
                        "user_question_answered",
                        &answered_payload,
                    );
                }
                Ok(UserQuestionOutcome {
                    question_id,
                    selected_index: row.selected_index.unwrap_or(0) as usize,
                    selected_option: row.selected_option.unwrap_or_default(),
                })
            }
            ApprovalResolution::Cancelled { reason } => {
                let event_type = if reason == "host_restart" {
                    "user_question_cancelled"
                } else {
                    "user_question_cancelled"
                };
                let payload = json!({
                    "questionId": question_id,
                    "waitingForUser": false,
                    "reason": reason,
                });
                if let Ok(receipt) = store
                    .append_run_event(&ctx.request_id, event_type, &payload)
                    .await
                {
                    let _ = events.emit_durable(receipt.id, event_type, &payload);
                }
                if reason == "host_restart" {
                    Err(UserQuestionError::Interrupted)
                } else {
                    Err(UserQuestionError::Cancelled)
                }
            }
            ApprovalResolution::Denied { reason } => Err(UserQuestionError::Internal(reason)),
            ApprovalResolution::Expired => {
                Err(UserQuestionError::Internal("question expired".into()))
            }
        }
    }

    async fn wait_for_resolution(
        &self,
        question_id: &str,
        run_id: &str,
        cancel: Arc<AtomicBool>,
        rx: tokio::sync::oneshot::Receiver<ApprovalResolution>,
    ) -> ApprovalResolution {
        let service = self.clone();
        let question_id_wait = question_id.to_string();
        let run_id_wait = run_id.to_string();
        let cancel_flag = cancel.clone();
        let cancel_watch = tokio::spawn(async move {
            while !cancel_flag.load(Ordering::Relaxed) {
                sleep(Duration::from_millis(50)).await;
            }
            let _ = service
                .resolve_pending(
                    &question_id_wait,
                    &run_id_wait,
                    "cancelled",
                    Some("run_cancelled"),
                    None,
                    None,
                )
                .await;
        });
        let resolution = rx.await.unwrap_or(ApprovalResolution::Denied {
            reason: "question waiter closed".into(),
        });
        cancel_watch.abort();
        resolution
    }
}

fn replay_existing(existing: UserQuestionRow) -> Result<UserQuestionOutcome, UserQuestionError> {
    match existing.status.as_str() {
        "answered" => Ok(UserQuestionOutcome {
            question_id: existing.id,
            selected_index: existing.selected_index.unwrap_or(0) as usize,
            selected_option: existing.selected_option.unwrap_or_default(),
        }),
        "pending" => Err(UserQuestionError::DuplicatePending),
        "interrupted" => Err(UserQuestionError::Interrupted),
        _ => Err(UserQuestionError::Cancelled),
    }
}

fn parse_options(value: &serde_json::Value) -> Result<Vec<String>, crate::error::ApiError> {
    value
        .as_array()
        .ok_or_else(|| crate::error::ApiError::Internal("question options missing".into()))?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_string)
                .ok_or_else(|| crate::error::ApiError::Internal("question option invalid".into()))
        })
        .collect()
}

pub struct RunScopedUserQuestion {
    service: UserQuestionService,
    store: Arc<dyn RunStore>,
    events: Arc<dyn EventSink>,
    cancel: Arc<AtomicBool>,
}

impl RunScopedUserQuestion {
    pub fn new(
        service: UserQuestionService,
        store: Arc<dyn RunStore>,
        events: Arc<dyn EventSink>,
        cancel: Arc<AtomicBool>,
    ) -> Arc<dyn AgentUserQuestion> {
        Arc::new(Self {
            service,
            store,
            events,
            cancel,
        })
    }
}

#[async_trait]
impl AgentUserQuestion for RunScopedUserQuestion {
    async fn ask_and_wait(
        &self,
        ctx: &UserQuestionContext,
        request: &UserQuestionRequest,
        cancel: &AtomicBool,
    ) -> Result<UserQuestionOutcome, UserQuestionError> {
        let _ = cancel;
        self.service
            .ask_and_wait(
                ctx,
                request,
                self.cancel.clone(),
                self.store.clone(),
                self.events.clone(),
            )
            .await
    }
}
