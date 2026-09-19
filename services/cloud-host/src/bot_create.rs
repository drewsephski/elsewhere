use agent_core::{
    BotCreateResult, CollaborationContext, CollaborationError, MAX_AGENT_CREATED_BOTS_PER_ROOT,
};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::api::bots::{validate_bot_instructions, validate_bot_name};
use crate::bot_avatar::normalize_avatar_id;
use crate::db::resources::{insert_bot, BotRow};
use crate::error::ApiError;

const COMPUTER_UNAVAILABLE: &str =
    "This Bot's computer is no longer available. Move it to a computer in Settings before creating a teammate.";

fn db_error(error: sqlx::Error) -> CollaborationError {
    CollaborationError::Internal(error.to_string())
}

fn map_api_error(err: ApiError) -> CollaborationError {
    match err {
        ApiError::NotFound => CollaborationError::NotFound,
        ApiError::Validation(m) => CollaborationError::Validation(m),
        ApiError::Conflict(m) => CollaborationError::Conflict(m),
        ApiError::Internal(m) => CollaborationError::Internal(m),
        other => CollaborationError::Internal(other.to_string()),
    }
}

fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(
        err,
        sqlx::Error::Database(db) if db.code().as_deref() == Some("23505")
    )
}

fn request_fingerprint(name: &str, instructions: &str, avatar_id: &str) -> String {
    format!("{name}|{instructions}|{avatar_id}")
}

pub async fn create_bot(
    pool: &PgPool,
    ctx: &CollaborationContext,
    name: &str,
    instructions: &str,
    avatar_id: Option<&str>,
) -> Result<BotCreateResult, CollaborationError> {
    if ctx.tool_invocation_id.is_empty() || ctx.tool_invocation_id.len() > 200 {
        return Err(CollaborationError::Validation(
            "tool invocation id is missing or too long".into(),
        ));
    }
    let name = validate_bot_name(name).map_err(map_api_error)?;
    let instructions = instructions.trim();
    let instructions = validate_bot_instructions(instructions, true).map_err(map_api_error)?;
    let avatar_id = normalize_avatar_id(avatar_id).map_err(map_api_error)?;
    let fingerprint = request_fingerprint(&name, &instructions, &avatar_id);

    let mut tx = pool.begin().await.map_err(db_error)?;

    let source_row =
        sqlx::query("SELECT owner_id, bot_id FROM agent_runs WHERE id = $1 AND owner_id = $2")
            .bind(&ctx.source_run_id)
            .bind(&ctx.owner_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_error)?
            .ok_or(CollaborationError::NotFound)?;

    if source_row.get::<String, _>("bot_id") != ctx.source_bot_id {
        return Err(CollaborationError::Internal(
            "source run bot mismatch".into(),
        ));
    }

    let (root_run_id, _, _) =
        crate::delegation::resolve_delegation_chain(&mut tx, &ctx.source_run_id).await?;

    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(format!("bot-create-root:{root_run_id}"))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

    if let Some(result) = load_creation_by_invocation(
        &mut tx,
        &ctx.source_run_id,
        &ctx.tool_invocation_id,
        &fingerprint,
    )
    .await?
    {
        tx.commit().await.map_err(db_error)?;
        return Ok(result);
    }

    let source_bot = sqlx::query_as::<_, BotRow>(
        r#"
        SELECT id, owner_id, name, system_prompt, model, computer_id, engine_preference,
               avatar_id, learn_from_conversations, created_at, updated_at
        FROM bots WHERE id = $1 AND owner_id = $2
        "#,
    )
    .bind(&ctx.source_bot_id)
    .bind(&ctx.owner_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(db_error)?
    .ok_or(CollaborationError::NotFound)?;

    let computer_id = source_bot
        .computer_id
        .clone()
        .ok_or_else(|| CollaborationError::Validation(COMPUTER_UNAVAILABLE.into()))?;
    let sandbox_state: Option<String> =
        sqlx::query_scalar("SELECT state FROM sandboxes WHERE id = $1 AND owner_id = $2")
            .bind(&computer_id)
            .bind(&ctx.owner_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_error)?;
    match sandbox_state.as_deref() {
        Some(state) if state != "archived" => {}
        _ => {
            return Err(CollaborationError::Validation(COMPUTER_UNAVAILABLE.into()));
        }
    }

    let created_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM bot_creations WHERE root_run_id = $1")
            .bind(&root_run_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(db_error)?;
    if created_count >= MAX_AGENT_CREATED_BOTS_PER_ROOT {
        return Err(CollaborationError::LimitExceeded(format!(
            "At most {} Bots may be created per root assignment",
            MAX_AGENT_CREATED_BOTS_PER_ROOT
        )));
    }

    sqlx::query("SAVEPOINT bot_create_insert")
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

    let created = insert_bot(
        &mut *tx,
        &ctx.owner_id,
        &name,
        &instructions,
        &source_bot.model,
        Some(&computer_id),
        &source_bot.engine_preference,
        &avatar_id,
    )
    .await
    .map_err(map_api_error)?;

    let creation_id = Uuid::new_v4().to_string();
    let insert = sqlx::query(
        r#"
        INSERT INTO bot_creations (
            id, owner_id, source_bot_id, source_run_id, root_run_id, created_bot_id,
            tool_invocation_id, request_fingerprint, name, instructions, avatar_id
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
        "#,
    )
    .bind(&creation_id)
    .bind(&ctx.owner_id)
    .bind(&ctx.source_bot_id)
    .bind(&ctx.source_run_id)
    .bind(&root_run_id)
    .bind(&created.id)
    .bind(&ctx.tool_invocation_id)
    .bind(&fingerprint)
    .bind(&name)
    .bind(&instructions)
    .bind(&avatar_id)
    .execute(&mut *tx)
    .await;

    if let Err(err) = insert {
        if is_unique_violation(&err) {
            sqlx::query("ROLLBACK TO SAVEPOINT bot_create_insert")
                .execute(&mut *tx)
                .await
                .map_err(db_error)?;
            if let Some(result) = load_creation_by_invocation(
                &mut tx,
                &ctx.source_run_id,
                &ctx.tool_invocation_id,
                &fingerprint,
            )
            .await?
            {
                tx.commit().await.map_err(db_error)?;
                return Ok(result);
            }
            return Err(db_error(err));
        }
        return Err(db_error(err));
    }

    tx.commit().await.map_err(db_error)?;
    Ok(BotCreateResult {
        bot_id: created.id,
        name: created.name,
        computer_id,
    })
}

async fn load_creation_by_invocation(
    tx: &mut Transaction<'_, Postgres>,
    source_run_id: &str,
    tool_invocation_id: &str,
    fingerprint: &str,
) -> Result<Option<BotCreateResult>, CollaborationError> {
    let existing = sqlx::query(
        r#"
        SELECT request_fingerprint, created_bot_id
        FROM bot_creations
        WHERE source_run_id = $1 AND tool_invocation_id = $2
        "#,
    )
    .bind(source_run_id)
    .bind(tool_invocation_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?;

    let Some(existing) = existing else {
        return Ok(None);
    };
    let stored_fingerprint: String = existing.get("request_fingerprint");
    if stored_fingerprint != fingerprint {
        return Err(CollaborationError::Conflict(
            "This tool call already created a different Bot".into(),
        ));
    }
    let created_bot_id: String = existing.get("created_bot_id");
    let created = sqlx::query("SELECT name, computer_id FROM bots WHERE id = $1")
        .bind(&created_bot_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(db_error)?
        .ok_or_else(|| CollaborationError::Internal("created Bot missing".into()))?;
    let computer_id: Option<String> = created.get("computer_id");
    Ok(Some(BotCreateResult {
        bot_id: created_bot_id,
        name: created.get("name"),
        computer_id: computer_id.unwrap_or_default(),
    }))
}
