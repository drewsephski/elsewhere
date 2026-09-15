use agent_core::{
    CreateRunParams, MessageRole, MessageStatus, PersistedMessage, RunEventReceipt, RunStore,
    RuntimeError, StructuredMessageInput,
};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::auth::LEGACY_LOCAL_OWNER;

pub struct PostgresRunStore {
    pool: PgPool,
}

impl PostgresRunStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

}

async fn next_message_sequence_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    conversation_id: &str,
) -> Result<i64, sqlx::Error> {
    let lock_key = format!("conversation-seq:{conversation_id}");
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(&lock_key)
        .execute(&mut **tx)
        .await?;
    let row: (Option<i64>,) = sqlx::query_as(
        "SELECT MAX(sequence) FROM messages WHERE conversation_id = $1",
    )
    .bind(conversation_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(row.0.unwrap_or(0) + 1)
}

#[async_trait]
impl RunStore for PostgresRunStore {
    async fn create_run(&self, params: CreateRunParams) -> Result<String, RuntimeError> {
        let run_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        sqlx::query(
            r#"
            INSERT INTO agent_runs (
                id, owner_id, request_id, bot_id, conversation_id, computer_id, model,
                status, step_count, started_at, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, 'running', 0, $8, $8, $8)
            "#,
        )
        .bind(&run_id)
        .bind(LEGACY_LOCAL_OWNER)
        .bind(&params.request_id)
        .bind(&params.bot_id)
        .bind(&params.conversation_id)
        .bind(&params.computer_id)
        .bind(&params.model)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| RuntimeError::Store(e.to_string()))?;
        Ok(run_id)
    }

    async fn append_run_event(
        &self,
        request_id: &str,
        event_type: &str,
        payload: &Value,
    ) -> Result<RunEventReceipt, RuntimeError> {
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO run_events (request_id, event_type, payload_json)
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
        )
        .bind(request_id)
        .bind(event_type)
        .bind(payload)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| RuntimeError::Store(e.to_string()))?;
        Ok(RunEventReceipt { id: row.0 })
    }

    async fn persist_structured_message(
        &self,
        input: StructuredMessageInput,
    ) -> Result<PersistedMessage, RuntimeError> {
        let message_id = Uuid::new_v4().to_string();
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| RuntimeError::Store(e.to_string()))?;
        let sequence = next_message_sequence_in_tx(&mut tx, &input.conversation_id)
            .await
            .map_err(|e| RuntimeError::Store(e.to_string()))?;
        let now = Utc::now();
        let role = role_to_str(input.role);
        let status = status_to_str(input.status);
        sqlx::query(
            r#"
            INSERT INTO messages (
                id, conversation_id, role, kind, body, status, model, sequence, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
            "#,
        )
        .bind(&message_id)
        .bind(&input.conversation_id)
        .bind(role)
        .bind(&input.kind)
        .bind(&input.body)
        .bind(status)
        .bind(&input.model)
        .bind(sequence)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|e| RuntimeError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| RuntimeError::Store(e.to_string()))?;
        Ok(PersistedMessage {
            id: message_id,
            conversation_id: input.conversation_id,
            kind: input.kind,
            body: input.body,
        })
    }

    async fn update_assistant_message(
        &self,
        message_id: &str,
        body: &str,
        status: MessageStatus,
        error_message: Option<&str>,
    ) -> Result<(), RuntimeError> {
        let now = Utc::now();
        sqlx::query(
            r#"
            UPDATE messages
            SET body = $1, status = $2, error_message = $3, updated_at = $4
            WHERE id = $5
            "#,
        )
        .bind(body)
        .bind(status_to_str(status))
        .bind(error_message)
        .bind(now)
        .bind(message_id)
        .execute(&self.pool)
        .await
        .map_err(|e| RuntimeError::Store(e.to_string()))?;
        Ok(())
    }

    async fn update_run(
        &self,
        request_id: &str,
        status: &str,
        error_code: Option<&str>,
        step_count: i64,
    ) -> Result<(), RuntimeError> {
        let now = Utc::now();
        let finished = crate::run_lifecycle::is_terminal_run_status(status);
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| RuntimeError::Store(e.to_string()))?;

        sqlx::query(
            r#"
            UPDATE agent_runs
            SET status = $1,
                error_code = $2,
                step_count = $3,
                updated_at = $4,
                finished_at = CASE WHEN $5 THEN COALESCE(finished_at, $4) ELSE finished_at END
            WHERE request_id = $6
            "#,
        )
        .bind(status)
        .bind(error_code)
        .bind(step_count)
        .bind(now)
        .bind(finished)
        .bind(request_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| RuntimeError::Store(e.to_string()))?;

        if finished {
            let run_row = sqlx::query("SELECT id FROM agent_runs WHERE request_id = $1")
                .bind(request_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| RuntimeError::Store(e.to_string()))?;
            if let Some(run_row) = run_row {
                let run_id: String = run_row.get("id");
                if let Err(err) =
                    crate::result_finalization::mark_terminal_run_results_policy(
                        &mut tx,
                        &run_id,
                        status,
                    )
                    .await
                {
                    tracing::warn!(
                        request_id = %request_id,
                        error = %err,
                        "could not set result finalization policy"
                    );
                }
                if status == "completed" {
                    if let Err(err) =
                        crate::conversation::commit_group_context_cursor_for_completed_run_in_tx(
                            &mut tx,
                            request_id,
                        )
                        .await
                    {
                        tracing::warn!(
                            request_id = %request_id,
                            error = %err,
                            "could not commit group context cursor"
                        );
                    }
                }
                if let Err(err) = crate::run_lifecycle::synchronize_run_terminal_in_tx(
                    &mut tx,
                    &run_id,
                    status,
                    error_code,
                )
                .await
                {
                    tracing::warn!(
                        request_id = %request_id,
                        error = %err,
                        "could not synchronize run terminal lifecycle"
                    );
                }
            }
        }

        tx.commit()
            .await
            .map_err(|e| RuntimeError::Store(e.to_string()))?;
        Ok(())
    }

    async fn touch_conversation_and_bot(
        &self,
        conversation_id: &str,
        bot_id: &str,
    ) -> Result<(), RuntimeError> {
        let now = Utc::now();
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| RuntimeError::Store(e.to_string()))?;
        sqlx::query("UPDATE conversations SET updated_at = $1 WHERE id = $2")
            .bind(now)
            .bind(conversation_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| RuntimeError::Store(e.to_string()))?;
        sqlx::query("UPDATE bots SET updated_at = $1 WHERE id = $2")
            .bind(now)
            .bind(bot_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| RuntimeError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| RuntimeError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_assistant_message_body(&self, message_id: &str) -> Result<String, RuntimeError> {
        let row: (String,) = sqlx::query_as("SELECT body FROM messages WHERE id = $1")
            .bind(message_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| RuntimeError::Store(e.to_string()))?;
        Ok(row.0)
    }

    async fn get_codex_thread_id(
        &self,
        conversation_id: &str,
        bot_id: &str,
    ) -> Result<Option<String>, RuntimeError> {
        crate::conversation::get_codex_thread_id(&self.pool, conversation_id, bot_id)
            .await
            .map_err(|e| RuntimeError::Store(e.to_string()))
    }

    async fn set_codex_thread_id(
        &self,
        conversation_id: &str,
        bot_id: &str,
        thread_id: &str,
    ) -> Result<(), RuntimeError> {
        crate::conversation::set_codex_thread_id(&self.pool, conversation_id, bot_id, thread_id)
            .await
            .map_err(|e| RuntimeError::Store(e.to_string()))
    }

    async fn clear_codex_thread_id(
        &self,
        conversation_id: &str,
        bot_id: &str,
    ) -> Result<(), RuntimeError> {
        crate::conversation::clear_codex_thread_id(&self.pool, conversation_id, bot_id)
            .await
            .map_err(|e| RuntimeError::Store(e.to_string()))
    }

    async fn count_completed_assistant_turns(
        &self,
        conversation_id: &str,
        bot_id: &str,
    ) -> Result<i64, RuntimeError> {
        crate::conversation::count_completed_assistant_turns(&self.pool, conversation_id, bot_id)
            .await
            .map_err(|e| RuntimeError::Store(e.to_string()))
    }

    async fn get_codex_compacted_through_turns(
        &self,
        conversation_id: &str,
        bot_id: &str,
    ) -> Result<i64, RuntimeError> {
        crate::conversation::get_codex_compacted_through_turns(&self.pool, conversation_id, bot_id)
            .await
            .map_err(|e| RuntimeError::Store(e.to_string()))
    }

    async fn set_codex_compacted_through_turns(
        &self,
        conversation_id: &str,
        bot_id: &str,
        turns: i64,
    ) -> Result<(), RuntimeError> {
        crate::conversation::set_codex_compacted_through_turns(
            &self.pool,
            conversation_id,
            bot_id,
            turns,
        )
        .await
        .map_err(|e| RuntimeError::Store(e.to_string()))
    }
}

fn role_to_str(role: MessageRole) -> &'static str {
    match role {
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::System => "system",
    }
}

fn status_to_str(status: MessageStatus) -> &'static str {
    match status {
        MessageStatus::Pending => "pending",
        MessageStatus::Streaming => "streaming",
        MessageStatus::Complete => "complete",
        MessageStatus::Error => "error",
        MessageStatus::Cancelled => "cancelled",
        MessageStatus::Interrupted => "interrupted",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::DEFAULT_MODEL;
    use serde_json::json;

    async fn try_test_pool() -> Option<PgPool> {
        let url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://elsewhere:elsewhere@127.0.0.1:5432/elsewhere".into());
        let pool = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            PgPool::connect(&url),
        )
        .await
        .ok()?
        .ok()?;
        sqlx::migrate!("./migrations").run(&pool).await.ok()?;
        Some(pool)
    }

    #[tokio::test]
    async fn postgres_run_store_roundtrip() {
        let Some(pool) = try_test_pool().await else {
            eprintln!("skipping postgres_run_store_roundtrip: Postgres unavailable");
            return;
        };
        let store = PostgresRunStore::new(pool.clone());

        let bot_id = Uuid::new_v4().to_string();
        let conv_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO bots (id, owner_id, name, system_prompt, model) VALUES ($1, 'legacy-local', 't', '', $2)",
        )
        .bind(&bot_id)
        .bind(DEFAULT_MODEL)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO conversations (id, owner_id, bot_id) VALUES ($1, 'legacy-local', $2)")
            .bind(&conv_id)
            .bind(&bot_id)
            .execute(&pool)
            .await
            .unwrap();

        let request_id = Uuid::new_v4().to_string();
        let run_id = store
            .create_run(CreateRunParams {
                conversation_id: conv_id.clone(),
                request_id: request_id.clone(),
                bot_id: bot_id.clone(),
                model: DEFAULT_MODEL.into(),
                computer_id: Some("computer_demo".into()),
            })
            .await
            .unwrap();

        store
            .append_run_event(&request_id, "status", &json!({"status":"running"}))
            .await
            .unwrap();

        let msg = store
            .persist_structured_message(StructuredMessageInput {
                conversation_id: conv_id.clone(),
                role: MessageRole::Assistant,
                kind: "tool_call".into(),
                body: "{}".into(),
                status: MessageStatus::Complete,
                model: Some(DEFAULT_MODEL.into()),
            })
            .await
            .unwrap();

        store
            .update_assistant_message(&msg.id, "hello", MessageStatus::Complete, None)
            .await
            .unwrap();
        store
            .update_run(&request_id, "completed", None, 1)
            .await
            .unwrap();

        let row: (String,) =
            sqlx::query_as("SELECT status FROM agent_runs WHERE id = $1")
                .bind(&run_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(row.0, "completed");

        let events: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM run_events WHERE request_id = $1")
                .bind(&request_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(events.0, 1);
    }
}
