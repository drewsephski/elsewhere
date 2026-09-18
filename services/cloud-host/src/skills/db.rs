use agent_skills::{SkillPackage, SkillPackageFile};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::ApiError;

fn db_err(e: sqlx::Error) -> ApiError {
    ApiError::Internal(e.to_string())
}

fn is_foreign_key_violation(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(|db| db.code())
        .is_some_and(|code| code == "23503")
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(|db| db.code())
        .is_some_and(|code| code == "23505")
}

#[derive(Debug, Clone)]
pub struct SkillRow {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub status: String,
    pub current_version: i32,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct SkillVersionRow {
    pub id: String,
    pub skill_id: String,
    pub version: i32,
    pub skill_md: String,
    pub content_hash: String,
    pub parsed_name: String,
    pub parsed_description: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_skills(pool: &PgPool, owner: &str) -> Result<Vec<SkillRow>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, slug, name, description, status, current_version, updated_at FROM skills WHERE owner_id = $1 ORDER BY updated_at DESC",
    )
    .bind(owner)
    .fetch_all(pool)
    .await
    .map_err(db_err)?;
    Ok(rows
        .into_iter()
        .map(|r| SkillRow {
            id: r.get("id"),
            slug: r.get("slug"),
            name: r.get("name"),
            description: r.get("description"),
            status: r.get("status"),
            current_version: r.get("current_version"),
            updated_at: r.get("updated_at"),
        })
        .collect())
}

pub async fn get_skill_for_owner(
    pool: &PgPool,
    owner: &str,
    skill_id: &str,
) -> Result<Option<SkillRow>, ApiError> {
    let row = sqlx::query(
        "SELECT id, slug, name, description, status, current_version, updated_at FROM skills WHERE id = $1 AND owner_id = $2",
    )
    .bind(skill_id)
    .bind(owner)
    .fetch_optional(pool)
    .await
    .map_err(db_err)?;
    Ok(row.map(|r| SkillRow {
        id: r.get("id"),
        slug: r.get("slug"),
        name: r.get("name"),
        description: r.get("description"),
        status: r.get("status"),
        current_version: r.get("current_version"),
        updated_at: r.get("updated_at"),
    }))
}

pub async fn create_skill_with_version(
    pool: &PgPool,
    owner: &str,
    slug: &str,
    package: &SkillPackage,
    extra_files: &[SkillPackageFile],
) -> Result<(SkillRow, SkillVersionRow), ApiError> {
    create_skill_with_version_and_attach(pool, owner, slug, package, extra_files, None).await
}

pub async fn create_skill_with_version_and_attach(
    pool: &PgPool,
    owner: &str,
    slug: &str,
    package: &SkillPackage,
    extra_files: &[SkillPackageFile],
    attach_bot_id: Option<&str>,
) -> Result<(SkillRow, SkillVersionRow), ApiError> {
    let skill_id = Uuid::new_v4().to_string();
    let version_id = Uuid::new_v4().to_string();
    let mut tx = pool.begin().await.map_err(db_err)?;
    sqlx::query(
        "INSERT INTO skills (id, owner_id, slug, name, description, status, current_version) VALUES ($1,$2,$3,$4,$5,'active',1)",
    )
    .bind(&skill_id)
    .bind(owner)
    .bind(slug)
    .bind(&package.frontmatter.name)
    .bind(&package.frontmatter.description)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            ApiError::Conflict(format!(
                "A skill with slug \"{slug}\" already exists for this owner"
            ))
        } else {
            db_err(e)
        }
    })?;
    insert_version_files(
        &mut tx,
        owner,
        &skill_id,
        &version_id,
        1,
        package,
        extra_files,
    )
    .await?;
    sqlx::query("UPDATE skills SET current_version = 1, updated_at = NOW() WHERE id = $1")
        .bind(&skill_id)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;

    if let Some(bot_id) = attach_bot_id {
        let bot_ok: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM bots WHERE id = $1 AND owner_id = $2)",
        )
        .bind(bot_id)
        .bind(owner)
        .fetch_one(&mut *tx)
        .await
        .map_err(db_err)?;
        if !bot_ok {
            return Err(ApiError::NotFound);
        }
        sqlx::query(
            "INSERT INTO bot_skills (bot_id, skill_id, owner_id, pinned_version, enabled) VALUES ($1,$2,$3,NULL,TRUE) ON CONFLICT (bot_id, skill_id) DO UPDATE SET pinned_version = EXCLUDED.pinned_version, enabled = TRUE, updated_at = NOW()",
        )
        .bind(bot_id)
        .bind(&skill_id)
        .bind(owner)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
    }

    tx.commit().await.map_err(db_err)?;
    let skill = get_skill_for_owner(pool, owner, &skill_id)
        .await?
        .expect("skill");
    let version = get_version_for_owner(pool, owner, &skill_id, 1)
        .await?
        .expect("version");
    Ok((skill, version))
}

pub async fn append_skill_version(
    pool: &PgPool,
    owner: &str,
    skill_id: &str,
    package: &SkillPackage,
    extra_files: &[SkillPackageFile],
) -> Result<SkillVersionRow, ApiError> {
    let skill = get_skill_for_owner(pool, owner, skill_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if skill.status == "archived" {
        return Err(ApiError::Validation(
            "Archived skills cannot receive new versions".into(),
        ));
    }
    let next_version = skill.current_version + 1;
    let version_id = Uuid::new_v4().to_string();
    let mut tx = pool.begin().await.map_err(db_err)?;
    insert_version_files(
        &mut tx,
        owner,
        skill_id,
        &version_id,
        next_version,
        package,
        extra_files,
    )
    .await?;
    sqlx::query(
        "UPDATE skills SET current_version = $2, name = $3, description = $4, updated_at = NOW() WHERE id = $1 AND owner_id = $5",
    )
    .bind(skill_id)
    .bind(next_version)
    .bind(&package.frontmatter.name)
    .bind(&package.frontmatter.description)
    .bind(owner)
    .execute(&mut *tx)
    .await
    .map_err(db_err)?;
    tx.commit().await.map_err(db_err)?;
    get_version_for_owner(pool, owner, skill_id, next_version)
        .await?
        .ok_or(ApiError::Internal("version missing".into()))
}

async fn insert_version_files(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &str,
    skill_id: &str,
    version_id: &str,
    version: i32,
    package: &SkillPackage,
    extra_files: &[SkillPackageFile],
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO skill_versions (id, skill_id, owner_id, version, skill_md, content_hash, parsed_name, parsed_description) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
    )
    .bind(version_id)
    .bind(skill_id)
    .bind(owner)
    .bind(version)
    .bind(&package.skill_md)
    .bind(&package.content_hash)
    .bind(&package.frontmatter.name)
    .bind(&package.frontmatter.description)
    .execute(&mut **tx)
    .await
    .map_err(db_err)?;
    for file in extra_files {
        let file_id = Uuid::new_v4().to_string();
        let size = file.content.len() as i32;
        let sha = format!("{:x}", Sha256::digest(file.content.as_bytes()));
        sqlx::query(
            "INSERT INTO skill_files (id, skill_version_id, relative_path, content, content_type, size, sha256) VALUES ($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(&file_id)
        .bind(version_id)
        .bind(&file.relative_path)
        .bind(&file.content)
        .bind(&file.content_type)
        .bind(size)
        .bind(&sha)
        .execute(&mut **tx)
        .await
        .map_err(db_err)?;
    }
    Ok(())
}

pub async fn get_version_for_owner(
    pool: &PgPool,
    owner: &str,
    skill_id: &str,
    version: i32,
) -> Result<Option<SkillVersionRow>, ApiError> {
    let row = sqlx::query(
        "SELECT id, skill_id, version, skill_md, content_hash, parsed_name, parsed_description, created_at FROM skill_versions WHERE skill_id = $1 AND owner_id = $2 AND version = $3",
    )
    .bind(skill_id)
    .bind(owner)
    .bind(version)
    .fetch_optional(pool)
    .await
    .map_err(db_err)?;
    Ok(row.map(|r| SkillVersionRow {
        id: r.get("id"),
        skill_id: r.get("skill_id"),
        version: r.get("version"),
        skill_md: r.get("skill_md"),
        content_hash: r.get("content_hash"),
        parsed_name: r.get("parsed_name"),
        parsed_description: r.get("parsed_description"),
        created_at: r.get("created_at"),
    }))
}

pub async fn list_version_package_files(
    pool: &PgPool,
    owner: &str,
    skill_id: &str,
    version: i32,
) -> Result<Vec<SkillPackageFile>, ApiError> {
    let version_row = get_version_for_owner(pool, owner, skill_id, version)
        .await?
        .ok_or(ApiError::NotFound)?;
    let file_rows = sqlx::query(
        "SELECT relative_path, content, content_type FROM skill_files WHERE skill_version_id = $1 ORDER BY relative_path ASC",
    )
    .bind(&version_row.id)
    .fetch_all(pool)
    .await
    .map_err(db_err)?;
    Ok(file_rows
        .into_iter()
        .map(|f| SkillPackageFile {
            relative_path: f.get("relative_path"),
            content: f.get("content"),
            content_type: f.get("content_type"),
        })
        .collect())
}

pub async fn list_versions(
    pool: &PgPool,
    owner: &str,
    skill_id: &str,
) -> Result<Vec<SkillVersionRow>, ApiError> {
    if get_skill_for_owner(pool, owner, skill_id).await?.is_none() {
        return Err(ApiError::NotFound);
    }
    let rows = sqlx::query(
        "SELECT id, skill_id, version, skill_md, content_hash, parsed_name, parsed_description, created_at FROM skill_versions WHERE skill_id = $1 AND owner_id = $2 ORDER BY version DESC",
    )
    .bind(skill_id)
    .bind(owner)
    .fetch_all(pool)
    .await
    .map_err(db_err)?;
    Ok(rows
        .into_iter()
        .map(|r| SkillVersionRow {
            id: r.get("id"),
            skill_id: r.get("skill_id"),
            version: r.get("version"),
            skill_md: r.get("skill_md"),
            content_hash: r.get("content_hash"),
            parsed_name: r.get("parsed_name"),
            parsed_description: r.get("parsed_description"),
            created_at: r.get("created_at"),
        })
        .collect())
}

pub async fn patch_skill_metadata(
    pool: &PgPool,
    owner: &str,
    skill_id: &str,
    name: Option<&str>,
    description: Option<&str>,
    status: Option<&str>,
) -> Result<SkillRow, ApiError> {
    let existing = get_skill_for_owner(pool, owner, skill_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let name = name.unwrap_or(&existing.name);
    let description = description.unwrap_or(&existing.description);
    let status = status.unwrap_or(&existing.status);
    if status != "active" && status != "archived" {
        return Err(ApiError::Validation("invalid skill status".into()));
    }
    sqlx::query(
        "UPDATE skills SET name = $2, description = $3, status = $4, updated_at = NOW() WHERE id = $1 AND owner_id = $5",
    )
    .bind(skill_id)
    .bind(name)
    .bind(description)
    .bind(status)
    .bind(owner)
    .execute(pool)
    .await
    .map_err(db_err)?;
    get_skill_for_owner(pool, owner, skill_id)
        .await?
        .ok_or(ApiError::Internal("skill missing".into()))
}

pub async fn delete_skill(pool: &PgPool, owner: &str, skill_id: &str) -> Result<(), ApiError> {
    let result = sqlx::query("DELETE FROM skills WHERE id = $1 AND owner_id = $2")
        .bind(skill_id)
        .bind(owner)
        .execute(pool)
        .await;
    match result {
        Ok(deleted) if deleted.rows_affected() == 0 => Err(ApiError::NotFound),
        Ok(_) => Ok(()),
        Err(error) if is_foreign_key_violation(&error) => Err(ApiError::Conflict(
            "This skill cannot be deleted because it was used in past work or a routine.".into(),
        )),
        Err(error) => Err(db_err(error)),
    }
}

pub async fn attach_bot_skill(
    pool: &PgPool,
    owner: &str,
    bot_id: &str,
    skill_id: &str,
    pinned_version: Option<i32>,
) -> Result<(), ApiError> {
    let skill = get_skill_for_owner(pool, owner, skill_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if skill.status == "archived" {
        return Err(ApiError::Validation(
            "Archived skills cannot be attached".into(),
        ));
    }
    if let Some(v) = pinned_version {
        if get_version_for_owner(pool, owner, skill_id, v)
            .await?
            .is_none()
        {
            return Err(ApiError::Validation("pinned version not found".into()));
        }
    }
    let bot_ok: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM bots WHERE id = $1 AND owner_id = $2)")
            .bind(bot_id)
            .bind(owner)
            .fetch_one(pool)
            .await
            .map_err(db_err)?;
    if !bot_ok {
        return Err(ApiError::NotFound);
    }
    sqlx::query(
        "INSERT INTO bot_skills (bot_id, skill_id, owner_id, pinned_version, enabled) VALUES ($1,$2,$3,$4,TRUE) ON CONFLICT (bot_id, skill_id) DO UPDATE SET pinned_version = EXCLUDED.pinned_version, enabled = TRUE, updated_at = NOW()",
    )
    .bind(bot_id)
    .bind(skill_id)
    .bind(owner)
    .bind(pinned_version)
    .execute(pool)
    .await
    .map_err(db_err)?;
    Ok(())
}

pub async fn detach_bot_skill(
    pool: &PgPool,
    owner: &str,
    bot_id: &str,
    skill_id: &str,
) -> Result<(), ApiError> {
    let result =
        sqlx::query("DELETE FROM bot_skills WHERE bot_id = $1 AND skill_id = $2 AND owner_id = $3")
            .bind(bot_id)
            .bind(skill_id)
            .bind(owner)
            .execute(pool)
            .await
            .map_err(db_err)?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BotSkillAttachment {
    pub skill_id: String,
    pub slug: String,
    pub name: String,
    pub enabled: bool,
    pub pinned_version: Option<i32>,
    pub current_version: i32,
}

pub async fn list_bot_skills(
    pool: &PgPool,
    owner: &str,
    bot_id: &str,
) -> Result<Vec<BotSkillAttachment>, ApiError> {
    let bot_ok: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM bots WHERE id = $1 AND owner_id = $2)")
            .bind(bot_id)
            .bind(owner)
            .fetch_one(pool)
            .await
            .map_err(db_err)?;
    if !bot_ok {
        return Err(ApiError::NotFound);
    }
    let rows = sqlx::query(
        r#"
        SELECT bs.skill_id, bs.enabled, bs.pinned_version, s.slug, s.name, s.current_version
        FROM bot_skills bs
        JOIN skills s ON s.id = bs.skill_id AND s.owner_id = bs.owner_id
        WHERE bs.bot_id = $1 AND bs.owner_id = $2
        ORDER BY s.name ASC
        "#,
    )
    .bind(bot_id)
    .bind(owner)
    .fetch_all(pool)
    .await
    .map_err(db_err)?;
    Ok(rows
        .into_iter()
        .map(|r| BotSkillAttachment {
            skill_id: r.get("skill_id"),
            slug: r.get("slug"),
            name: r.get("name"),
            enabled: r.get("enabled"),
            pinned_version: r.get("pinned_version"),
            current_version: r.get("current_version"),
        })
        .collect())
}
