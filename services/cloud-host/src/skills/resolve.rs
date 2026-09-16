use std::collections::HashMap;

use sqlx::{Row, Transaction};

use crate::error::ApiError;

#[derive(Debug, Clone, Default)]
pub struct ExplicitSkillInvocation {
    pub skill_id: String,
    pub version: Option<i32>,
}

#[derive(Debug, Clone, Default)]
pub struct SkillAdmissionInput {
    pub explicit: Option<ExplicitSkillInvocation>,
    pub routine_skill_id: Option<String>,
    pub routine_pinned_version: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InvocationKind {
    Attached,
    Explicit,
    Routine,
}

struct ResolvedSkill {
    skill_id: String,
    version: i32,
    kind: InvocationKind,
}

fn db_err(e: sqlx::Error) -> ApiError {
    ApiError::Internal(e.to_string())
}

pub async fn persist_run_skills_for_admission(
    tx: &mut Transaction<'_, sqlx::Postgres>,
    owner: &str,
    bot_id: &str,
    run_id: &str,
    input: &SkillAdmissionInput,
) -> Result<(), ApiError> {
    let mut resolved: HashMap<String, ResolvedSkill> = HashMap::new();

    let attached = sqlx::query(
        r#"
        SELECT bs.skill_id, bs.pinned_version, s.current_version, s.status
        FROM bot_skills bs
        JOIN skills s ON s.id = bs.skill_id AND s.owner_id = bs.owner_id
        WHERE bs.bot_id = $1 AND bs.owner_id = $2 AND bs.enabled = TRUE
        "#,
    )
    .bind(bot_id)
    .bind(owner)
    .fetch_all(&mut **tx)
    .await
    .map_err(db_err)?;

    for row in attached {
        let skill_id: String = row.get("skill_id");
        let status: String = row.get("status");
        if status != "active" {
            continue;
        }
        let pinned: Option<i32> = row.get("pinned_version");
        let current: i32 = row.get("current_version");
        let version = pinned.unwrap_or(current);
        resolved.insert(
            skill_id.clone(),
            ResolvedSkill {
                skill_id,
                version,
                kind: InvocationKind::Attached,
            },
        );
    }

    if let Some(routine_id) = input.routine_skill_id.as_deref() {
        let row = sqlx::query(
            "SELECT id, current_version, status FROM skills WHERE id = $1 AND owner_id = $2",
        )
        .bind(routine_id)
        .bind(owner)
        .fetch_optional(&mut **tx)
        .await
        .map_err(db_err)?;
        let Some(row) = row else {
            return Err(ApiError::Validation("routine skill not found".into()));
        };
        if row.get::<String, _>("status") != "active" {
            return Err(ApiError::Validation("routine skill is not active".into()));
        }
        let current: i32 = row.get("current_version");
        let version = input.routine_pinned_version.unwrap_or(current);
        resolved.insert(
            routine_id.to_string(),
            ResolvedSkill {
                skill_id: routine_id.to_string(),
                version,
                kind: InvocationKind::Routine,
            },
        );
    }

    if let Some(explicit) = &input.explicit {
        let row = sqlx::query(
            "SELECT id, current_version, status FROM skills WHERE id = $1 AND owner_id = $2",
        )
        .bind(&explicit.skill_id)
        .bind(owner)
        .fetch_optional(&mut **tx)
        .await
        .map_err(db_err)?;
        let Some(row) = row else {
            return Err(ApiError::NotFound);
        };
        if row.get::<String, _>("status") != "active" {
            return Err(ApiError::Validation(
                "skill is not available for invocation".into(),
            ));
        }
        let current: i32 = row.get("current_version");
        let version = explicit.version.unwrap_or(current);
        resolved.insert(
            explicit.skill_id.clone(),
            ResolvedSkill {
                skill_id: explicit.skill_id.clone(),
                version,
                kind: InvocationKind::Explicit,
            },
        );
    }

    for entry in resolved.values() {
        let version_row = sqlx::query(
            "SELECT id FROM skill_versions WHERE skill_id = $1 AND owner_id = $2 AND version = $3",
        )
        .bind(&entry.skill_id)
        .bind(owner)
        .bind(entry.version)
        .fetch_optional(&mut **tx)
        .await
        .map_err(db_err)?;
        let Some(version_row) = version_row else {
            return Err(ApiError::Validation("skill version not found".into()));
        };
        let version_id: String = version_row.get("id");
        let kind = match entry.kind {
            InvocationKind::Attached => "attached",
            InvocationKind::Explicit => "explicit",
            InvocationKind::Routine => "routine",
        };
        sqlx::query(
            "INSERT INTO run_skills (run_id, skill_id, skill_version_id, invocation_kind) VALUES ($1,$2,$3,$4) ON CONFLICT (run_id, skill_id) DO UPDATE SET skill_version_id = EXCLUDED.skill_version_id, invocation_kind = EXCLUDED.invocation_kind",
        )
        .bind(run_id)
        .bind(&entry.skill_id)
        .bind(&version_id)
        .bind(kind)
        .execute(&mut **tx)
        .await
        .map_err(db_err)?;
    }

    Ok(())
}

/// Explicit skill selection stored on a run (concrete version number).
pub async fn explicit_invocation_on_run(
    tx: &mut Transaction<'_, sqlx::Postgres>,
    run_id: &str,
) -> Result<Option<(String, i32)>, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT rs.skill_id, sv.version
        FROM run_skills rs
        JOIN skill_versions sv ON sv.id = rs.skill_version_id
        WHERE rs.run_id = $1 AND rs.invocation_kind = 'explicit'
        LIMIT 1
        "#,
    )
    .bind(run_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_err)?;
    Ok(row.map(|r| (r.get("skill_id"), r.get("version"))))
}

pub fn explicit_invocation_idempotency_mismatch(
    requested: &SkillAdmissionInput,
    stored: Option<(String, i32)>,
) -> bool {
    match (&requested.explicit, stored) {
        (None, None) => false,
        (None, Some(_)) | (Some(_), None) => true,
        (Some(req), Some((skill_id, version))) => {
            if req.skill_id != skill_id {
                return true;
            }
            if let Some(pinned) = req.version {
                return pinned != version;
            }
            false
        }
    }
}

pub async fn resolve_explicit_skill_id(
    tx: &mut Transaction<'_, sqlx::Postgres>,
    owner: &str,
    skill_id_or_slug: &str,
) -> Result<String, ApiError> {
    if let Some(id) =
        sqlx::query_scalar::<_, String>("SELECT id FROM skills WHERE id = $1 AND owner_id = $2")
            .bind(skill_id_or_slug)
            .bind(owner)
            .fetch_optional(&mut **tx)
            .await
            .map_err(db_err)?
    {
        return Ok(id);
    }
    sqlx::query_scalar::<_, String>("SELECT id FROM skills WHERE slug = $1 AND owner_id = $2")
        .bind(skill_id_or_slug)
        .bind(owner)
        .fetch_optional(&mut **tx)
        .await
        .map_err(db_err)?
        .ok_or(ApiError::NotFound)
}
