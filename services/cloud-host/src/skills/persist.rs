//! Persist reviewed skill packages (conversation UI + bot tools).

use agent_core::SkillDraftFile;
use agent_skills::{SkillPackage, SkillPackageFile};

use sqlx::PgPool;

use crate::error::ApiError;

use super::db::{
    create_skill_with_version_and_attach, list_skills, SkillRow, SkillVersionRow,
};
use super::draft::{apply_skill_save_overrides, title_to_skill_slug};

pub fn skill_package_files_from_draft(files: &[SkillDraftFile]) -> Vec<SkillPackageFile> {
    files
        .iter()
        .map(|f| SkillPackageFile {
            relative_path: f.relative_path.clone(),
            content: f.content.clone(),
            content_type: f.content_type.clone(),
        })
        .collect()
}

pub fn draft_files_from_package(files: &[SkillPackageFile]) -> Vec<SkillDraftFile> {
    files
        .iter()
        .map(|f| SkillDraftFile {
            relative_path: f.relative_path.clone(),
            content: f.content.clone(),
            content_type: f.content_type.clone(),
        })
        .collect()
}

/// Validate and persist the exact reviewed package (optional attach in the same transaction).
pub async fn save_reviewed_skill_package(
    pool: &PgPool,
    owner: &str,
    skill_md: &str,
    files: &[SkillPackageFile],
    optional_name: Option<&str>,
    optional_description: Option<&str>,
    attach_bot_id: Option<&str>,
) -> Result<(SkillRow, SkillVersionRow), ApiError> {
    let initial = SkillPackage::validate_and_build(skill_md, files, None)
        .map_err(|e| ApiError::Validation(e.to_string()))?;
    let (package, package_files) =
        apply_skill_save_overrides(&initial, files, optional_name, optional_description)?;
    let slug = package.frontmatter.name.clone();

    let existing = list_skills(pool, owner).await?;
    if existing
        .iter()
        .any(|row| row.slug == slug && row.status == "active")
    {
        return Err(ApiError::Conflict(format!(
            "A skill named \"{slug}\" already exists. Choose a different name or update the existing skill on the Skills page."
        )));
    }

    create_skill_with_version_and_attach(
        pool,
        owner,
        &slug,
        &package,
        &package_files,
        attach_bot_id,
    )
    .await
}

pub fn normalize_skill_title(name: &str) -> Result<String, ApiError> {
    title_to_skill_slug(name)
}
