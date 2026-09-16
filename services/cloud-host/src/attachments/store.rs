use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use agent_core::{
    attachment_workspace_path, AttachmentDescriptor, AttachmentKind, MAX_ATTACHMENTS_PER_MESSAGE,
    MAX_ATTACHMENT_TOTAL_BYTES, STAGED_ATTACHMENT_TTL_HOURS,
};

use super::validate::ValidatedUpload;
use crate::error::ApiError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AttachmentRow {
    pub id: String,
    pub owner_id: String,
    pub bot_id: Option<String>,
    pub conversation_id: Option<String>,
    pub original_name: String,
    pub safe_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub sha256: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct StagedAttachment {
    pub meta: AttachmentRow,
    pub content: Vec<u8>,
}

fn db_error(error: sqlx::Error) -> ApiError {
    ApiError::Internal(error.to_string())
}

pub async fn insert_staged(
    pool: &PgPool,
    owner_id: &str,
    bot_id: Option<&str>,
    conversation_id: Option<&str>,
    validated: &ValidatedUpload,
    content: &[u8],
) -> Result<AttachmentRow, ApiError> {
    let id = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + chrono::Duration::hours(STAGED_ATTACHMENT_TTL_HOURS);
    sqlx::query(
        r#"
        INSERT INTO attachments (
            id, owner_id, bot_id, conversation_id, original_name, safe_name,
            mime_type, size_bytes, sha256, content, status, expires_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'staged',$11)
        "#,
    )
    .bind(&id)
    .bind(owner_id)
    .bind(bot_id)
    .bind(conversation_id)
    .bind(&validated.original_name)
    .bind(&validated.safe_name)
    .bind(&validated.mime_type)
    .bind(validated.size_bytes as i64)
    .bind(&validated.sha256)
    .bind(content)
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(db_error)?;

    load_meta(pool, owner_id, &id)
        .await?
        .ok_or_else(|| ApiError::Internal("staged attachment missing after insert".into()))
}

pub async fn load_meta(
    pool: &PgPool,
    owner_id: &str,
    attachment_id: &str,
) -> Result<Option<AttachmentRow>, ApiError> {
    sqlx::query_as(
        r#"
        SELECT id, owner_id, bot_id, conversation_id, original_name, safe_name,
               mime_type, size_bytes, sha256, status, created_at, expires_at
        FROM attachments
        WHERE id = $1 AND owner_id = $2 AND status <> 'deleted'
        "#,
    )
    .bind(attachment_id)
    .bind(owner_id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)
}

pub async fn load_content_for_owner(
    pool: &PgPool,
    owner_id: &str,
    attachment_id: &str,
) -> Result<Option<(AttachmentRow, Vec<u8>)>, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT id, owner_id, bot_id, conversation_id, original_name, safe_name,
               mime_type, size_bytes, sha256, status, created_at, expires_at, content
        FROM attachments
        WHERE id = $1 AND owner_id = $2 AND status IN ('staged', 'attached')
        "#,
    )
    .bind(attachment_id)
    .bind(owner_id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    Ok(Some((row_from_query(&row), row.get("content"))))
}

fn row_from_query(row: &sqlx::postgres::PgRow) -> AttachmentRow {
    AttachmentRow {
        id: row.get("id"),
        owner_id: row.get("owner_id"),
        bot_id: row.get("bot_id"),
        conversation_id: row.get("conversation_id"),
        original_name: row.get("original_name"),
        safe_name: row.get("safe_name"),
        mime_type: row.get("mime_type"),
        size_bytes: row.get("size_bytes"),
        sha256: row.get("sha256"),
        status: row.get("status"),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
    }
}

pub async fn delete_staged(
    pool: &PgPool,
    owner_id: &str,
    attachment_id: &str,
) -> Result<bool, ApiError> {
    let result = sqlx::query(
        "UPDATE attachments SET status = 'deleted', hidden_at = NOW() WHERE id = $1 AND owner_id = $2 AND status = 'staged'",
    )
    .bind(attachment_id)
    .bind(owner_id)
    .execute(pool)
    .await
    .map_err(db_error)?;
    Ok(result.rows_affected() == 1)
}

pub async fn hide_attached(
    pool: &PgPool,
    owner_id: &str,
    attachment_id: &str,
) -> Result<bool, ApiError> {
    let result = sqlx::query(
        "UPDATE attachments SET hidden_at = NOW() WHERE id = $1 AND owner_id = $2 AND status = 'attached' AND hidden_at IS NULL",
    )
    .bind(attachment_id)
    .bind(owner_id)
    .execute(pool)
    .await
    .map_err(db_error)?;
    Ok(result.rows_affected() == 1)
}

pub async fn expire_unused_staged(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM attachments WHERE status = 'staged' AND expires_at IS NOT NULL AND expires_at < NOW()",
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

pub async fn load_staged_for_admission(
    tx: &mut Transaction<'_, Postgres>,
    owner_id: &str,
    bot_id: Option<&str>,
    conversation_id: Option<&str>,
    attachment_ids: &[String],
) -> Result<Vec<StagedAttachment>, ApiError> {
    if attachment_ids.len() > MAX_ATTACHMENTS_PER_MESSAGE {
        return Err(ApiError::Validation(
            "At most 4 attachments can be sent with a message".into(),
        ));
    }
    let mut seen = std::collections::HashSet::new();
    let mut loaded = Vec::new();
    let mut total: u64 = 0;
    for id in attachment_ids {
        if !seen.insert(id) {
            return Err(ApiError::Validation(
                "Duplicate attachment ids are not allowed".into(),
            ));
        }
        let row = sqlx::query(
            r#"
            SELECT id, owner_id, bot_id, conversation_id, original_name, safe_name,
                   mime_type, size_bytes, sha256, status, created_at, expires_at, content
            FROM attachments
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(db_error)?
        .ok_or(ApiError::NotFound)?;
        let meta = row_from_query(&row);
        if meta.owner_id != owner_id {
            return Err(ApiError::NotFound);
        }
        if meta.status != "staged" {
            return Err(ApiError::Validation(
                "That attachment is not available to send".into(),
            ));
        }
        if meta.expires_at.is_some_and(|expires| expires <= Utc::now()) {
            return Err(ApiError::Validation("That upload has expired".into()));
        }
        if let Some(expected_bot) = bot_id {
            if meta.bot_id.as_deref() != Some(expected_bot) {
                return Err(ApiError::Validation(
                    "That attachment belongs to a different Bot".into(),
                ));
            }
        }
        if let Some(expected_conversation) = conversation_id {
            if let Some(attached_conversation) = meta.conversation_id.as_deref() {
                if attached_conversation != expected_conversation {
                    return Err(ApiError::Validation(
                        "That attachment belongs to a different conversation".into(),
                    ));
                }
            }
        }
        total = total.saturating_add(meta.size_bytes as u64);
        if total > MAX_ATTACHMENT_TOTAL_BYTES {
            return Err(ApiError::Validation(
                "Attachments exceed the 20 MiB combined limit".into(),
            ));
        }
        loaded.push(StagedAttachment {
            meta,
            content: row.get("content"),
        });
    }
    Ok(loaded)
}

pub async fn attach_to_message_and_run(
    tx: &mut Transaction<'_, Postgres>,
    message_id: &str,
    run_id: &str,
    attachments: &[StagedAttachment],
) -> Result<Vec<AttachmentDescriptor>, ApiError> {
    let mut descriptors = Vec::new();
    for (ordinal, item) in attachments.iter().enumerate() {
        let ordinal_i = ordinal as i32;
        let workspace_path =
            attachment_workspace_path(run_id, ordinal_i, &item.meta.id, &item.meta.mime_type);
        sqlx::query(
            "INSERT INTO message_attachments (message_id, attachment_id, ordinal) VALUES ($1,$2,$3)",
        )
        .bind(message_id)
        .bind(&item.meta.id)
        .bind(ordinal_i)
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
        sqlx::query(
            r#"
            INSERT INTO run_attachments (
                run_id, attachment_id, ordinal, original_name, safe_name, mime_type, size_bytes, sha256, workspace_path
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
            "#,
        )
        .bind(run_id)
        .bind(&item.meta.id)
        .bind(ordinal_i)
        .bind(&item.meta.original_name)
        .bind(&item.meta.safe_name)
        .bind(&item.meta.mime_type)
        .bind(item.meta.size_bytes)
        .bind(&item.meta.sha256)
        .bind(&workspace_path)
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
        sqlx::query(
            "UPDATE attachments SET status = 'attached', expires_at = NULL WHERE id = $1 AND status = 'staged'",
        )
        .bind(&item.meta.id)
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
        descriptors.push(descriptor_from_meta(&item.meta, &workspace_path));
    }
    Ok(descriptors)
}

pub async fn attach_to_message_only(
    tx: &mut Transaction<'_, Postgres>,
    message_id: &str,
    attachments: &[StagedAttachment],
) -> Result<(), ApiError> {
    for (ordinal, item) in attachments.iter().enumerate() {
        sqlx::query(
            "INSERT INTO message_attachments (message_id, attachment_id, ordinal) VALUES ($1,$2,$3)",
        )
        .bind(message_id)
        .bind(&item.meta.id)
        .bind(ordinal as i32)
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
        sqlx::query(
            "UPDATE attachments SET status = 'attached', expires_at = NULL WHERE id = $1 AND status = 'staged'",
        )
        .bind(&item.meta.id)
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
    }
    Ok(())
}

pub async fn snapshot_message_attachments_onto_run(
    tx: &mut Transaction<'_, Postgres>,
    message_id: &str,
    run_id: &str,
) -> Result<Vec<AttachmentDescriptor>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT a.id, a.original_name, a.safe_name, a.mime_type, a.size_bytes, a.sha256, ma.ordinal
        FROM message_attachments ma
        JOIN attachments a ON a.id = ma.attachment_id
        WHERE ma.message_id = $1
        ORDER BY ma.ordinal ASC
        "#,
    )
    .bind(message_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(db_error)?;
    let mut descriptors = Vec::new();
    for row in rows {
        let id: String = row.get("id");
        let mime: String = row.get("mime_type");
        let ordinal: i32 = row.get("ordinal");
        let workspace_path = attachment_workspace_path(run_id, ordinal, &id, &mime);
        sqlx::query(
            r#"
            INSERT INTO run_attachments (
                run_id, attachment_id, ordinal, original_name, safe_name, mime_type, size_bytes, sha256, workspace_path
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
            ON CONFLICT (run_id, attachment_id) DO NOTHING
            "#,
        )
        .bind(run_id)
        .bind(&id)
        .bind(ordinal)
        .bind(row.get::<String, _>("original_name"))
        .bind(row.get::<String, _>("safe_name"))
        .bind(&mime)
        .bind(row.get::<i64, _>("size_bytes"))
        .bind(row.get::<String, _>("sha256"))
        .bind(&workspace_path)
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
        descriptors.push(AttachmentDescriptor {
            id,
            original_name: row.get("original_name"),
            safe_name: row.get("safe_name"),
            mime_type: mime,
            size_bytes: row.get::<i64, _>("size_bytes") as u64,
            sha256: row.get("sha256"),
            workspace_path,
            kind: AttachmentKind::from_mime(&row.get::<String, _>("mime_type"))
                .unwrap_or(AttachmentKind::Text),
        });
    }
    Ok(descriptors)
}

pub async fn attachment_ids_for_run_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    run_id: &str,
) -> Result<Vec<String>, ApiError> {
    sqlx::query_scalar(
        "SELECT attachment_id FROM run_attachments WHERE run_id = $1 ORDER BY ordinal ASC",
    )
    .bind(run_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(db_error)
}

pub async fn list_for_run(
    pool: &PgPool,
    run_id: &str,
) -> Result<Vec<AttachmentDescriptor>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT attachment_id, original_name, safe_name, mime_type, size_bytes, sha256, workspace_path, ordinal
        FROM run_attachments
        WHERE run_id = $1
        ORDER BY ordinal ASC
        "#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let mime: String = row.get("mime_type");
            AttachmentDescriptor {
                id: row.get("attachment_id"),
                original_name: row.get("original_name"),
                safe_name: row.get("safe_name"),
                mime_type: mime.clone(),
                size_bytes: row.get::<i64, _>("size_bytes") as u64,
                sha256: row.get("sha256"),
                workspace_path: row.get("workspace_path"),
                kind: AttachmentKind::from_mime(&mime).unwrap_or(AttachmentKind::Text),
            }
        })
        .collect())
}

pub async fn list_for_runs(
    pool: &PgPool,
    run_ids: &[String],
) -> Result<std::collections::HashMap<String, Vec<AttachmentDescriptor>>, ApiError> {
    let mut map = std::collections::HashMap::<String, Vec<AttachmentDescriptor>>::new();
    if run_ids.is_empty() {
        return Ok(map);
    }
    let rows = sqlx::query(
        r#"
        SELECT run_id, attachment_id, original_name, safe_name, mime_type, size_bytes, sha256, workspace_path, ordinal
        FROM run_attachments
        WHERE run_id = ANY($1)
        ORDER BY run_id ASC, ordinal ASC
        "#,
    )
    .bind(run_ids)
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    for row in rows {
        let mime: String = row.get("mime_type");
        let run_id: String = row.get("run_id");
        map.entry(run_id).or_default().push(AttachmentDescriptor {
            id: row.get("attachment_id"),
            original_name: row.get("original_name"),
            safe_name: row.get("safe_name"),
            mime_type: mime.clone(),
            size_bytes: row.get::<i64, _>("size_bytes") as u64,
            sha256: row.get("sha256"),
            workspace_path: row.get("workspace_path"),
            kind: AttachmentKind::from_mime(&mime).unwrap_or(AttachmentKind::Text),
        });
    }
    Ok(map)
}

pub async fn load_run_attachment_bytes(
    pool: &PgPool,
    run_id: &str,
    attachment_id: &str,
) -> Result<Option<(AttachmentDescriptor, Vec<u8>)>, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT ra.attachment_id, ra.original_name, ra.safe_name, ra.mime_type, ra.size_bytes,
               ra.sha256, ra.workspace_path, a.content
        FROM run_attachments ra
        JOIN attachments a ON a.id = ra.attachment_id
        WHERE ra.run_id = $1 AND ra.attachment_id = $2
        "#,
    )
    .bind(run_id)
    .bind(attachment_id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let mime: String = row.get("mime_type");
    Ok(Some((
        AttachmentDescriptor {
            id: row.get("attachment_id"),
            original_name: row.get("original_name"),
            safe_name: row.get("safe_name"),
            mime_type: mime.clone(),
            size_bytes: row.get::<i64, _>("size_bytes") as u64,
            sha256: row.get("sha256"),
            workspace_path: row.get("workspace_path"),
            kind: AttachmentKind::from_mime(&mime).unwrap_or(AttachmentKind::Text),
        },
        row.get("content"),
    )))
}

pub async fn list_for_messages(
    pool: &PgPool,
    message_ids: &[String],
) -> Result<std::collections::HashMap<String, Vec<AttachmentDescriptor>>, ApiError> {
    if message_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let rows = sqlx::query(
        r#"
        SELECT ma.message_id, a.id, a.original_name, a.safe_name, a.mime_type, a.size_bytes, a.sha256, ma.ordinal
        FROM message_attachments ma
        JOIN attachments a ON a.id = ma.attachment_id
        WHERE ma.message_id = ANY($1)
        ORDER BY ma.message_id, ma.ordinal
        "#,
    )
    .bind(message_ids)
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    let mut map: std::collections::HashMap<String, Vec<AttachmentDescriptor>> =
        std::collections::HashMap::new();
    for row in rows {
        let message_id: String = row.get("message_id");
        let mime: String = row.get("mime_type");
        map.entry(message_id)
            .or_default()
            .push(AttachmentDescriptor {
                id: row.get("id"),
                original_name: row.get("original_name"),
                safe_name: row.get("safe_name"),
                mime_type: mime.clone(),
                size_bytes: row.get::<i64, _>("size_bytes") as u64,
                sha256: row.get("sha256"),
                workspace_path: String::new(),
                kind: AttachmentKind::from_mime(&mime).unwrap_or(AttachmentKind::Text),
            });
    }
    Ok(map)
}

fn descriptor_from_meta(meta: &AttachmentRow, workspace_path: &str) -> AttachmentDescriptor {
    AttachmentDescriptor {
        id: meta.id.clone(),
        original_name: meta.original_name.clone(),
        safe_name: meta.safe_name.clone(),
        mime_type: meta.mime_type.clone(),
        size_bytes: meta.size_bytes as u64,
        sha256: meta.sha256.clone(),
        workspace_path: workspace_path.to_string(),
        kind: AttachmentKind::from_mime(&meta.mime_type).unwrap_or(AttachmentKind::Text),
    }
}

pub fn fingerprint(ids: &[String]) -> String {
    ids.join(",")
}
