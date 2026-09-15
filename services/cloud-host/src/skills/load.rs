use agent_skills::{SkillPackage, SkillPackageFile};
use sqlx::{PgPool, Row};

use crate::error::ApiError;

pub async fn load_run_skill_packages(
    pool: &PgPool,
    owner: &str,
    run_id: &str,
) -> Result<Vec<SkillPackage>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT rs.skill_id, rs.skill_version_id, sv.skill_md, s.slug
        FROM run_skills rs
        JOIN skill_versions sv ON sv.id = rs.skill_version_id
        JOIN skills s ON s.id = rs.skill_id
        JOIN agent_runs r ON r.id = rs.run_id
        WHERE rs.run_id = $1 AND r.owner_id = $2
        ORDER BY rs.created_at ASC
        "#,
    )
    .bind(run_id)
    .bind(owner)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let mut packages = Vec::with_capacity(rows.len());
    for row in rows {
        let skill_md: String = row.get("skill_md");
        let slug: String = row.get("slug");
        let version_id: String = row.get("skill_version_id");

        let file_rows = sqlx::query(
            "SELECT relative_path, content, content_type FROM skill_files WHERE skill_version_id = $1 ORDER BY relative_path ASC",
        )
        .bind(&version_id)
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

        let files: Vec<SkillPackageFile> = file_rows
            .into_iter()
            .map(|f| SkillPackageFile {
                relative_path: f.get("relative_path"),
                content: f.get("content"),
                content_type: f.get("content_type"),
            })
            .collect();

        let package = SkillPackage::validate_and_build(&skill_md, &files, Some(&slug))
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        packages.push(package);
    }
    Ok(packages)
}
