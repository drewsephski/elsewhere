//! Durable product work admission and dispatch. Side effects are never replayed after interruption.
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    db::queries::BootstrapRunRecords,
    error::ApiError,
    runner::RunExecutionInput,
    skills::{persist_run_skills_for_admission, SkillAdmissionInput},
};

fn db_error(error: sqlx::Error) -> ApiError {
    ApiError::Internal(error.to_string())
}

pub async fn enqueue(
    pool: &PgPool,
    owner: &str,
    request_id: &str,
    bot_id: &str,
    conversation_id: Option<&str>,
    message: &str,
) -> Result<BootstrapRunRecords, ApiError> {
    enqueue_with_skills(
        pool,
        owner,
        request_id,
        bot_id,
        conversation_id,
        message,
        &SkillAdmissionInput::default(),
    )
    .await
}

pub async fn enqueue_with_skills(
    pool: &PgPool,
    owner: &str,
    request_id: &str,
    bot_id: &str,
    conversation_id: Option<&str>,
    message: &str,
    skills: &SkillAdmissionInput,
) -> Result<BootstrapRunRecords, ApiError> {
    let mut tx = pool.begin().await.map_err(db_error)?;
    let records = enqueue_in_transaction(
        &mut tx,
        owner,
        request_id,
        bot_id,
        conversation_id,
        message,
        skills,
    )
    .await?;
    tx.commit().await.map_err(db_error)?;
    Ok(records)
}

pub async fn enqueue_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &str,
    request_id: &str,
    bot_id: &str,
    conversation_id: Option<&str>,
    message: &str,
    skills: &SkillAdmissionInput,
) -> Result<BootstrapRunRecords, ApiError> {
    crate::work_admission::admit(
        tx,
        owner,
        request_id,
        crate::work_admission::WorkAdmissionIntent::HumanMessage {
            bot_id,
            conversation_id,
            message,
            skills,
        },
    )
    .await
}

/// Admit unattended routine work without creating a fake human-authored message.
pub async fn enqueue_routine_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &str,
    request_id: &str,
    bot_id: &str,
    conversation_id: &str,
    routine_id: &str,
    routine_name: &str,
    runtime_message: &str,
    conversation_is_group: bool,
) -> Result<BootstrapRunRecords, ApiError> {
    if runtime_message.trim().is_empty() || runtime_message.len() > 100_000 {
        return Err(ApiError::Validation(
            "Work must contain between 1 and 100,000 bytes".into(),
        ));
    }
    if request_id.len() > 200 {
        return Err(ApiError::Validation("Idempotency key is too long".into()));
    }
    for key in [
        format!("work-owner:{owner}"),
        format!("work-request:{request_id}"),
    ] {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
            .bind(key)
            .execute(&mut **tx)
            .await
            .map_err(db_error)?;
    }
    if let Some(row) = sqlx::query(
        "SELECT r.*, q.user_message, q.instructions FROM agent_runs r LEFT JOIN work_queue q ON q.run_id = r.id WHERE request_id = $1",
    )
    .bind(request_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    {
        if row.get::<String, _>("owner_id") != owner {
            return Err(ApiError::NotFound);
        }
        if row.get::<String, _>("bot_id") != bot_id
            || row.get::<Option<String>, _>("user_message").as_deref() != Some(runtime_message.trim())
            || row.get::<String, _>("conversation_id") != conversation_id
        {
            return Err(ApiError::Conflict(
                "This request key was already used for different work".into(),
            ));
        }
        return Ok(BootstrapRunRecords {
            run_id: row.get("id"),
            request_id: request_id.into(),
            conversation_id: row.get("conversation_id"),
            assistant_message_id: row.get("assistant_message_id"),
            computer_id: row.get("computer_id"),
            model: row.get("model"),
            instructions: row.get("instructions"),
            is_new_run: false,
        });
    }

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agent_runs WHERE owner_id = $1 AND status IN ('queued', 'running')",
    )
    .bind(owner)
    .fetch_one(&mut **tx)
    .await
    .map_err(db_error)?;
    if count >= 100 {
        return Err(ApiError::TooManyRequests);
    }

    crate::groups::assert_bot_may_use_conversation(tx, owner, bot_id, conversation_id).await?;

    let snapshot =
        crate::work_admission::load_bot_execution_snapshot(tx, owner, bot_id, runtime_message)
            .await?;
    let model = snapshot.model.clone();
    let bot_display_name = snapshot.bot_name.clone();
    let instructions = snapshot.instructions.clone();
    let computer_id = snapshot.computer_id.clone();
    let engine = snapshot.engine.clone();

    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(format!("conversation-seq:{conversation_id}"))
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
    let through_sequence: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sequence), 0) FROM messages WHERE conversation_id = $1",
    )
    .bind(conversation_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(db_error)?;

    let mut sequence = through_sequence;
    let source_message_id = if conversation_is_group {
        sequence += 1;
        let system_id = Uuid::new_v4().to_string();
        let system_body = format!("Routine \"{routine_name}\" started for {bot_display_name}.");
        let system_provenance = serde_json::json!({
            "kind": "routine",
            "routineId": routine_id,
            "routineName": routine_name,
            "phase": "started",
        });
        sqlx::query(
            r#"
            INSERT INTO messages (id, conversation_id, role, body, status, sequence, author_kind, provenance)
            VALUES ($1, $2, 'user', $3, 'complete', $4, 'system', $5)
            "#,
        )
        .bind(&system_id)
        .bind(conversation_id)
        .bind(&system_body)
        .bind(sequence)
        .bind(&system_provenance)
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
        Some(system_id)
    } else {
        None
    };

    sequence += 1;
    let assistant_message_id = Uuid::new_v4().to_string();
    let assistant_provenance = serde_json::json!({
        "kind": "routine",
        "routineId": routine_id,
        "routineName": routine_name,
    });
    sqlx::query(
        r#"
        INSERT INTO messages (
            id, conversation_id, role, body, status, sequence, model, author_kind, author_bot_id, provenance
        ) VALUES ($1, $2, 'assistant', '', 'pending', $3, $4, 'bot', $5, $6)
        "#,
    )
    .bind(&assistant_message_id)
    .bind(conversation_id)
    .bind(sequence)
    .bind(&model)
    .bind(bot_id)
    .bind(&assistant_provenance)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;

    let run_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO agent_runs (
            id, owner_id, request_id, bot_id, conversation_id, computer_id, model,
            status, assistant_message_id, source_message_id, group_context_through_sequence
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,'queued',$8,$9,$10)
        "#,
    )
    .bind(&run_id)
    .bind(owner)
    .bind(request_id)
    .bind(bot_id)
    .bind(conversation_id)
    .bind(&computer_id)
    .bind(&model)
    .bind(&assistant_message_id)
    .bind(source_message_id.as_deref())
    .bind(if conversation_is_group {
        Some(through_sequence)
    } else {
        None
    })
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;

    sqlx::query(
        r#"
        INSERT INTO work_queue (run_id, user_message, instructions, engine_preference, routine_id, provenance_kind)
        VALUES ($1,$2,$3,$4,$5,'routine')
        "#,
    )
    .bind(&run_id)
    .bind(runtime_message.trim())
    .bind(&instructions)
    .bind(&engine)
    .bind(routine_id)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;

    sqlx::query(
        "INSERT INTO run_events (request_id, event_type, payload_json) VALUES ($1, 'queued', $2)",
    )
    .bind(request_id)
    .bind(serde_json::json!({
        "status": "queued",
        "detail": "Routine work saved. Waiting for an available computer.",
        "provenanceKind": "routine",
        "routineId": routine_id,
    }))
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    sqlx::query("UPDATE conversations SET updated_at = NOW() WHERE id = $1")
        .bind(conversation_id)
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;

    let routine_skill = sqlx::query(
        "SELECT skill_id, pinned_skill_version FROM routines WHERE id = $1 AND owner_id = $2",
    )
    .bind(routine_id)
    .bind(owner)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?;
    let mut skill_admission = SkillAdmissionInput::default();
    if let Some(row) = routine_skill {
        skill_admission.routine_skill_id = row.get("skill_id");
        skill_admission.routine_pinned_version = row.get("pinned_skill_version");
    }
    persist_run_skills_for_admission(tx, owner, bot_id, &run_id, &skill_admission).await?;
    crate::memory::snapshot::persist_run_memories(tx, owner, bot_id, &run_id, &snapshot.memories)
        .await?;

    Ok(BootstrapRunRecords {
        run_id,
        request_id: request_id.into(),
        conversation_id: conversation_id.to_string(),
        assistant_message_id,
        computer_id,
        model,
        instructions,
        is_new_run: true,
    })
}

/// Enqueue a bot run against an existing group human message (no duplicate user transcript row).
pub async fn enqueue_from_group_message_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &str,
    request_id: &str,
    bot_id: &str,
    conversation_id: &str,
    source_message_id: &str,
    user_message: &str,
    skills: &SkillAdmissionInput,
) -> Result<BootstrapRunRecords, ApiError> {
    if user_message.trim().is_empty() || user_message.len() > 100_000 {
        return Err(ApiError::Validation(
            "Work must contain between 1 and 100,000 bytes".into(),
        ));
    }
    if request_id.len() > 200 {
        return Err(ApiError::Validation("Idempotency key is too long".into()));
    }
    for key in [
        format!("work-owner:{owner}"),
        format!("work-request:{request_id}"),
    ] {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
            .bind(key)
            .execute(&mut **tx)
            .await
            .map_err(db_error)?;
    }
    if let Some(row) = sqlx::query(
        "SELECT r.*, q.user_message, q.instructions FROM agent_runs r LEFT JOIN work_queue q ON q.run_id = r.id WHERE request_id = $1",
    )
    .bind(request_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    {
        if row.get::<String, _>("owner_id") != owner {
            return Err(ApiError::NotFound);
        }
        if row.get::<String, _>("bot_id") != bot_id
            || row.get::<Option<String>, _>("source_message_id").as_deref() != Some(source_message_id)
            || row.get::<Option<String>, _>("user_message").as_deref() != Some(user_message.trim())
        {
            return Err(ApiError::Conflict(
                "This request key was already used for different work".into(),
            ));
        }
        return Ok(BootstrapRunRecords {
            run_id: row.get("id"),
            request_id: request_id.into(),
            conversation_id: row.get("conversation_id"),
            assistant_message_id: row.get("assistant_message_id"),
            computer_id: row.get("computer_id"),
            model: row.get("model"),
            instructions: row.get("instructions"),
            is_new_run: false,
        });
    }

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agent_runs WHERE owner_id = $1 AND status IN ('queued', 'running')",
    )
    .bind(owner)
    .fetch_one(&mut **tx)
    .await
    .map_err(db_error)?;
    if count >= 100 {
        return Err(ApiError::TooManyRequests);
    }

    let source_row = sqlx::query(
        "SELECT body, sequence, conversation_id FROM messages WHERE id = $1 AND author_kind = 'human'",
    )
    .bind(source_message_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    .ok_or(ApiError::NotFound)?;
    if source_row.get::<String, _>("conversation_id") != conversation_id {
        return Err(ApiError::NotFound);
    }
    if source_row.get::<String, _>("body") != user_message.trim() {
        return Err(ApiError::Validation(
            "source message body does not match work payload".into(),
        ));
    }

    let skill_invocation: Option<serde_json::Value> =
        sqlx::query_scalar("SELECT skill_invocation FROM messages WHERE id = $1")
            .bind(source_message_id)
            .fetch_one(&mut **tx)
            .await
            .map_err(db_error)?;
    let mut skill_admission = skills.clone();
    if skill_admission.explicit.is_none() {
        if let Some(v) = skill_invocation {
            if let Some(skill_id) = v.get("skillId").and_then(|x| x.as_str()) {
                skill_admission.explicit = Some(crate::skills::ExplicitSkillInvocation {
                    skill_id: skill_id.to_string(),
                    version: v
                        .get("skillVersion")
                        .and_then(|x| x.as_i64())
                        .map(|n| n as i32),
                });
            }
        }
    }

    let source_sequence: i64 = source_row.get("sequence");

    crate::groups::assert_bot_may_use_conversation(tx, owner, bot_id, conversation_id).await?;

    let snapshot =
        crate::work_admission::load_bot_execution_snapshot(tx, owner, bot_id, user_message).await?;
    let model = snapshot.model.clone();
    let instructions = snapshot.instructions.clone();
    let computer_id = snapshot.computer_id.clone();
    let engine = snapshot.engine.clone();

    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(format!("conversation-seq:{conversation_id}"))
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
    let sequence: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sequence), 0) + 1 FROM messages WHERE conversation_id = $1",
    )
    .bind(conversation_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(db_error)?;

    let assistant_message_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO messages (id, conversation_id, role, body, status, sequence, model, author_kind, author_bot_id)
        VALUES ($1, $2, 'assistant', '', 'pending', $3, $4, 'bot', $5)
        "#,
    )
    .bind(&assistant_message_id)
    .bind(conversation_id)
    .bind(sequence)
    .bind(&model)
    .bind(bot_id)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;

    let run_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO agent_runs (id, owner_id, request_id, bot_id, conversation_id, computer_id, model, status, assistant_message_id, source_message_id, group_context_through_sequence) VALUES ($1,$2,$3,$4,$5,$6,$7,'queued',$8,$9,$10)",
    )
    .bind(&run_id)
    .bind(owner)
    .bind(request_id)
    .bind(bot_id)
    .bind(conversation_id)
    .bind(&computer_id)
    .bind(&model)
    .bind(&assistant_message_id)
    .bind(source_message_id)
    .bind(source_sequence)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    sqlx::query(
        "INSERT INTO work_queue (run_id, user_message, instructions, engine_preference) VALUES ($1,$2,$3,$4)",
    )
    .bind(&run_id)
    .bind(user_message.trim())
    .bind(&instructions)
    .bind(&engine)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    sqlx::query("INSERT INTO run_events (request_id, event_type, payload_json) VALUES ($1, 'queued', $2)")
        .bind(request_id)
        .bind(serde_json::json!({"status":"queued", "detail":"Work saved. Waiting for an available computer."}))
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
    sqlx::query("UPDATE conversations SET updated_at = NOW() WHERE id = $1")
        .bind(conversation_id)
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;

    persist_run_skills_for_admission(tx, owner, bot_id, &run_id, &skill_admission).await?;
    crate::memory::snapshot::persist_run_memories(tx, owner, bot_id, &run_id, &snapshot.memories)
        .await?;

    Ok(BootstrapRunRecords {
        run_id,
        request_id: request_id.into(),
        conversation_id: conversation_id.to_string(),
        assistant_message_id,
        computer_id,
        model,
        instructions,
        is_new_run: true,
    })
}

pub async fn enqueue_delegated_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &str,
    request_id: &str,
    bot_id: &str,
    conversation_id: Option<&str>,
    message: &str,
    delegation_id: &str,
) -> Result<BootstrapRunRecords, ApiError> {
    let records = enqueue_in_transaction(
        tx,
        owner,
        request_id,
        bot_id,
        conversation_id,
        message,
        &SkillAdmissionInput::default(),
    )
    .await?;
    sqlx::query(
        "UPDATE work_queue SET delegation_id = $2, provenance_kind = 'bot_delegation' WHERE run_id = $1",
    )
    .bind(&records.run_id)
    .bind(delegation_id)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    Ok(records)
}

const MAX_DELEGATION_RETURN_RESULT_CHARS: usize = 12_000;
async fn load_delegation_artifacts_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    delegation_id: &str,
) -> Result<Vec<crate::artifact_handoff::ArtifactContextLine>, ApiError> {
    crate::artifact_handoff::load_artifact_context_lines(&mut **tx, delegation_id)
        .await
        .map_err(db_error)
}

/// Queue exactly one source-Bot continuation after a delegated target reaches a terminal state.
pub async fn enqueue_delegation_return_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    delegation_id: &str,
) -> Result<Option<String>, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT d.*,
               tb.name AS target_bot_name,
               tr.status AS target_run_status,
               tr.id AS target_run_id,
               tr.error_code AS target_run_error_code,
               am.body AS target_result_body,
               sb.computer_id AS source_computer_id,
               tb.computer_id AS target_computer_id
        FROM bot_delegations d
        JOIN bots tb ON tb.id = d.target_bot_id
        JOIN bots sb ON sb.id = d.source_bot_id
        LEFT JOIN agent_runs tr ON tr.id = d.target_run_id
        LEFT JOIN messages am ON am.id = tr.assistant_message_id
        WHERE d.id = $1
        FOR UPDATE OF d
        "#,
    )
    .bind(delegation_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    .ok_or(ApiError::NotFound)?;

    let return_policy: String = row.get("return_policy");
    if return_policy != "resume_source" {
        return Ok(None);
    }
    let resume_status: Option<String> = row.get("resume_status");
    let source_resume_run_id: Option<String> = row.get("source_resume_run_id");
    let source_request_id: String = row.get("source_request_id");
    if source_resume_run_id.is_some() {
        return Ok(source_resume_run_id);
    }
    if resume_status.as_deref() == Some("skipped") {
        return Ok(None);
    }

    let return_request_id = format!("delegation-return:{delegation_id}");
    if let Some(existing_run_id) =
        sqlx::query_scalar::<_, String>("SELECT id FROM agent_runs WHERE request_id = $1")
            .bind(&return_request_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(db_error)?
    {
        sqlx::query(
            r#"
            UPDATE bot_delegations
            SET source_resume_run_id = $2,
                resume_status = COALESCE(resume_status, 'queued'),
                resume_created_at = COALESCE(resume_created_at, NOW())
            WHERE id = $1 AND source_resume_run_id IS NULL
            "#,
        )
        .bind(delegation_id)
        .bind(&existing_run_id)
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
        return Ok(Some(existing_run_id));
    }

    let owner: String = row.get("owner_id");
    let source_bot_id: String = row.get("source_bot_id");
    let source_conversation_id: String = row.get("source_conversation_id");
    let instruction: String = row.get("instruction");
    let target_bot_name: String = row.get("target_bot_name");
    let target_run_id: Option<String> = row.get("target_run_id");
    let target_run_status: Option<String> = row.get("target_run_status");
    let target_status = target_run_status.unwrap_or_else(|| "failed".into());
    let mut target_result: String = row
        .get::<Option<String>, _>("target_result_body")
        .unwrap_or_default();
    if target_result.len() > MAX_DELEGATION_RETURN_RESULT_CHARS {
        target_result.truncate(MAX_DELEGATION_RETURN_RESULT_CHARS);
        target_result.push('…');
    }

    let conv_type: String = sqlx::query_scalar(
        "SELECT conversation_type FROM conversations WHERE id = $1 AND owner_id = $2",
    )
    .bind(&source_conversation_id)
    .bind(&owner)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    .ok_or(ApiError::NotFound)?;

    if conv_type == "group" {
        let active: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
              SELECT 1 FROM conversation_participants
              WHERE conversation_id = $1 AND bot_id = $2 AND owner_id = $3 AND left_at IS NULL
            )
            "#,
        )
        .bind(&source_conversation_id)
        .bind(&source_bot_id)
        .bind(&owner)
        .fetch_optional(&mut **tx)
        .await
        .map_err(db_error)?
        .unwrap_or(false);
        if !active {
            sqlx::query(
                r#"
                UPDATE bot_delegations
                SET resume_status = 'skipped',
                    resume_error = 'source bot is not an active group participant',
                    resume_created_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(delegation_id)
            .execute(&mut **tx)
            .await
            .map_err(db_error)?;
            sqlx::query(
                "INSERT INTO run_events (request_id, event_type, payload_json) VALUES ($1, 'bot_delegation_return_skipped', $2)",
            )
            .bind(&source_request_id)
            .bind(serde_json::json!({
                "delegationId": delegation_id,
                "resumeStatus": "skipped",
                "reason": "source bot is not an active group participant"
            }))
            .execute(&mut **tx)
            .await
            .map_err(db_error)?;
            return Ok(None);
        }
    }

    let artifacts = load_delegation_artifacts_in_tx(tx, delegation_id).await?;
    let results_collection_note: Option<String> = if target_run_id.is_some() {
        sqlx::query_scalar(
            r#"
            SELECT CASE
              WHEN tr.results_status = 'partial' THEN COALESCE(tr.results_note, 'Result collection was partial.')
              WHEN tr.results_status = 'failed' THEN COALESCE(tr.results_note, 'Result collection failed.')
              ELSE NULL
            END
            FROM bot_delegations d
            JOIN agent_runs tr ON tr.id = d.target_run_id
            WHERE d.id = $1
            "#,
        )
        .bind(delegation_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(db_error)?
        .flatten()
    } else {
        None
    };
    let source_computer_id: String = row.get("source_computer_id");
    let target_computer_id: String = row.get("target_computer_id");
    let shared_computer =
        crate::run_lifecycle::bots_share_computer(&source_computer_id, &target_computer_id);
    let target_run_error: Option<String> = row.get("target_run_error_code");
    let interruption_detail = if target_status == "interrupted" {
        Some(match target_run_error.as_deref() {
            Some("host_restart") => {
                format!(
                    "{target_bot_name} was interrupted by a runner restart. Do not replay their external actions; continue from this summary."
                )
            }
            Some(code) => format!(
                "{target_bot_name} was interrupted ({code}). Continue from the summary without replaying their side effects."
            ),
            None => format!(
                "{target_bot_name} was interrupted. Continue from the summary without replaying their side effects."
            ),
        })
    } else {
        None
    };

    let user_message = crate::delegation::format_delegation_return_user_message(
        &target_bot_name,
        &instruction,
        &target_status,
        &target_result,
        target_run_id.as_deref().unwrap_or(""),
        &artifacts,
        results_collection_note.as_deref(),
        shared_computer,
        interruption_detail.as_deref(),
    );

    let request_id = return_request_id;
    for key in [
        format!("work-owner:{owner}"),
        format!("work-request:{request_id}"),
    ] {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
            .bind(key)
            .execute(&mut **tx)
            .await
            .map_err(db_error)?;
    }

    crate::groups::assert_bot_may_use_conversation(
        tx,
        &owner,
        &source_bot_id,
        &source_conversation_id,
    )
    .await?;

    let snapshot = crate::work_admission::load_bot_execution_snapshot(
        tx,
        &owner,
        &source_bot_id,
        &user_message,
    )
    .await
    .map_err(|err| match err {
        ApiError::Validation(_) => ApiError::Validation("Source bot computer unavailable".into()),
        other => other,
    })?;
    let model = snapshot.model.clone();
    let instructions = snapshot.instructions.clone();
    let computer_id = snapshot.computer_id.clone();
    let engine = snapshot.engine.clone();

    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(format!("conversation-seq:{source_conversation_id}"))
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
    let sequence: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sequence), 0) + 1 FROM messages WHERE conversation_id = $1",
    )
    .bind(&source_conversation_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(db_error)?;

    let system_message_id = Uuid::new_v4().to_string();
    let system_body = format!("Delegated work returned from {target_bot_name} ({target_status}).");
    let provenance = serde_json::json!({
        "kind": "delegation_return",
        "delegationId": delegation_id,
        "targetRunId": target_run_id,
        "targetStatus": target_status,
    });
    sqlx::query(
        r#"
        INSERT INTO messages (id, conversation_id, role, body, status, sequence, author_kind, provenance)
        VALUES ($1, $2, 'user', $3, 'complete', $4, 'system', $5)
        "#,
    )
    .bind(&system_message_id)
    .bind(&source_conversation_id)
    .bind(&system_body)
    .bind(sequence)
    .bind(&provenance)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;

    let assistant_message_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO messages (id, conversation_id, role, body, status, sequence, model, author_kind, author_bot_id)
        VALUES ($1, $2, 'assistant', '', 'pending', $3, $4, 'bot', $5)
        "#,
    )
    .bind(&assistant_message_id)
    .bind(&source_conversation_id)
    .bind(sequence + 1)
    .bind(&model)
    .bind(&source_bot_id)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;

    let run_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO agent_runs (
            id, owner_id, request_id, bot_id, conversation_id, computer_id, model,
            status, assistant_message_id, source_message_id
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,'queued',$8,$9)
        "#,
    )
    .bind(&run_id)
    .bind(&owner)
    .bind(&request_id)
    .bind(&source_bot_id)
    .bind(&source_conversation_id)
    .bind(&computer_id)
    .bind(&model)
    .bind(&assistant_message_id)
    .bind(&system_message_id)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;

    sqlx::query(
        r#"
        INSERT INTO work_queue (run_id, user_message, instructions, engine_preference, delegation_id, provenance_kind)
        VALUES ($1,$2,$3,$4,$5,'delegation_return')
        "#,
    )
    .bind(&run_id)
    .bind(&user_message)
    .bind(&instructions)
    .bind(&engine)
    .bind(delegation_id)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;

    sqlx::query(
        r#"
        UPDATE bot_delegations
        SET source_resume_run_id = $2,
            resume_status = 'queued',
            resume_created_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(delegation_id)
    .bind(&run_id)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;

    crate::memory::snapshot::persist_run_memories(
        tx,
        &owner,
        &source_bot_id,
        &run_id,
        &snapshot.memories,
    )
    .await?;

    sqlx::query(
        "INSERT INTO run_events (request_id, event_type, payload_json) VALUES ($1, 'queued', $2)",
    )
    .bind(&request_id)
    .bind(serde_json::json!({"status":"queued","provenanceKind":"delegation_return"}))
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;

    sqlx::query(
        "INSERT INTO run_events (request_id, event_type, payload_json) VALUES ($1, 'bot_delegation_return_queued', $2)",
    )
    .bind(&source_request_id)
    .bind(serde_json::json!({
        "delegationId": delegation_id,
        "sourceResumeRunId": run_id,
        "resumeStatus": "queued"
    }))
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;

    Ok(Some(run_id))
}

/// Atomic claim serializes all dispatchers, and only one work item uses a computer or bot at a time.
pub async fn claim_next(pool: &PgPool) -> Result<Option<RunExecutionInput>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(18493721)")
        .execute(&mut *tx)
        .await?;
    let row = sqlx::query("SELECT r.*, q.user_message, q.instructions, q.engine_preference FROM agent_runs r JOIN work_queue q ON q.run_id = r.id WHERE r.status = 'queued' AND NOT EXISTS(SELECT 1 FROM agent_runs active WHERE (active.status = 'running' OR (active.started_at IS NOT NULL AND active.execution_released_at IS NULL)) AND (active.computer_id = r.computer_id OR active.bot_id = r.bot_id)) ORDER BY r.created_at, r.id LIMIT 1 FOR UPDATE OF r SKIP LOCKED")
        .fetch_optional(&mut *tx).await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let run_id: String = row.get("id");
    sqlx::query("UPDATE agent_runs SET status = 'running', started_at = NOW(), updated_at = NOW() WHERE id = $1")
        .bind(&run_id).execute(&mut *tx).await?;
    sqlx::query(
        r#"
        UPDATE group_message_recipients
        SET status = 'running', updated_at = NOW()
        WHERE run_id = $1 AND status = 'queued'
        "#,
    )
    .bind(&run_id)
    .execute(&mut *tx)
    .await?;
    let input = RunExecutionInput {
        records: BootstrapRunRecords {
            run_id,
            request_id: row.get("request_id"),
            conversation_id: row.get("conversation_id"),
            assistant_message_id: row.get("assistant_message_id"),
            computer_id: row.get("computer_id"),
            model: row.get("model"),
            instructions: row.get("instructions"),
            is_new_run: true,
        },
        bot_id: row.get("bot_id"),
        user_message: row.get("user_message"),
        engine_mode: crate::run_engine_select::parse_run_engine_mode(row.get("engine_preference"))
            .ok(),
    };
    tx.commit().await?;
    if let Err(err) = crate::delegation::on_target_run_claimed(pool, &input.records.run_id).await {
        tracing::warn!(run_id = %input.records.run_id, error = %err, "could not mark delegation running");
    }
    if let Err(err) =
        crate::run_lifecycle::on_delegation_return_run_claimed(pool, &input.records.run_id).await
    {
        tracing::warn!(
            run_id = %input.records.run_id,
            error = %err,
            "could not mark delegation return running"
        );
    }
    if let Err(err) = crate::routine_runs::mark_running_for_run(pool, &input.records.run_id).await {
        tracing::warn!(
            run_id = %input.records.run_id,
            error = %err,
            "could not mark routine run running"
        );
    }
    Ok(Some(input))
}

pub async fn request_cancel(pool: &PgPool, owner: &str, run_id: &str) -> Result<(), ApiError> {
    let mut tx = pool.begin().await.map_err(db_error)?;
    let row = sqlx::query("SELECT status, request_id, assistant_message_id FROM agent_runs WHERE id = $1 AND owner_id = $2 FOR UPDATE")
        .bind(run_id).bind(owner).fetch_optional(&mut *tx).await.map_err(db_error)?.ok_or(ApiError::NotFound)?;
    let status: String = row.get("status");
    if status == "queued" {
        let request_id: String = row.get("request_id");
        sqlx::query("UPDATE agent_runs SET status = 'cancelled', cancel_requested = TRUE, finished_at = NOW(), updated_at = NOW() WHERE id = $1")
            .bind(run_id).execute(&mut *tx).await.map_err(db_error)?;
        sqlx::query("UPDATE messages SET status = 'cancelled', updated_at = NOW() WHERE id = $1")
            .bind(row.get::<String, _>("assistant_message_id"))
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        sqlx::query("INSERT INTO run_events (request_id, event_type, payload_json) VALUES ($1, 'cancelled', $2)")
            .bind(&request_id).bind(serde_json::json!({"status":"cancelled"})).execute(&mut *tx).await.map_err(db_error)?;
        if let Err(err) =
            crate::run_lifecycle::synchronize_run_terminal_in_tx(&mut tx, run_id, "cancelled", None)
                .await
        {
            tracing::warn!(
                run_id = %run_id,
                error = %err,
                "could not synchronize cancelled run lifecycle"
            );
        }
    } else if status == "running" {
        sqlx::query("UPDATE agent_runs SET cancel_requested = TRUE WHERE id = $1")
            .bind(run_id)
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
    }
    tx.commit().await.map_err(db_error)?;
    Ok(())
}
