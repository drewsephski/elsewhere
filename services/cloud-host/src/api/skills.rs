use agent_skills::{SkillPackage, SkillPackageFile};
use axum::extract::{Path, State};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::auth::Principal;
use crate::error::ApiError;
use crate::skills::{
    append_skill_version, attach_bot_skill, create_skill_with_version, delete_skill,
    detach_bot_skill, generate_skill_draft_from_run, get_skill_for_owner, get_version_for_owner,
    list_bot_skills, list_skills, list_version_package_files, list_versions, patch_skill_metadata,
    save_reviewed_skill_package, BotSkillAttachment, SkillRow, SkillVersionRow,
};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillFileInput {
    relative_path: String,
    content: String,
    content_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillPackageInput {
    skill_md: String,
    /// Omitted: inherit the current version's package files. Present (including `[]`): replace.
    files: Option<Vec<SkillFileInput>>,
}

fn files_from_input(files: &[SkillFileInput]) -> Vec<SkillPackageFile> {
    files
        .iter()
        .map(|f| SkillPackageFile {
            relative_path: f.relative_path.clone(),
            content: f.content.clone(),
            content_type: f.content_type.clone(),
        })
        .collect()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSummaryResponse {
    id: String,
    slug: String,
    name: String,
    description: String,
    status: String,
    current_version: i32,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<SkillRow> for SkillSummaryResponse {
    fn from(row: SkillRow) -> Self {
        Self {
            id: row.id,
            slug: row.slug,
            name: row.name,
            description: row.description,
            status: row.status,
            current_version: row.current_version,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillVersionResponse {
    id: String,
    skill_id: String,
    version: i32,
    content_hash: String,
    parsed_name: String,
    parsed_description: String,
    skill_md: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl From<SkillVersionRow> for SkillVersionResponse {
    fn from(row: SkillVersionRow) -> Self {
        Self {
            id: row.id,
            skill_id: row.skill_id,
            version: row.version,
            content_hash: row.content_hash,
            parsed_name: row.parsed_name,
            parsed_description: row.parsed_description,
            skill_md: row.skill_md,
            created_at: row.created_at,
        }
    }
}

pub async fn list(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
) -> Result<Json<Vec<SkillSummaryResponse>>, ApiError> {
    let rows = list_skills(&state.pool, principal.owner_id()).await?;
    Ok(Json(
        rows.into_iter().map(SkillSummaryResponse::from).collect(),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSkillRequest {
    pub slug: String,
    pub skill_md: String,
    #[serde(default)]
    pub files: Vec<SkillFileInput>,
}

pub async fn create(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(body): Json<CreateSkillRequest>,
) -> Result<Json<SkillSummaryResponse>, ApiError> {
    if body.slug.len() > 64 {
        return Err(ApiError::Validation("slug too long".into()));
    }
    let files = files_from_input(&body.files);
    let package = SkillPackage::validate_and_build(&body.skill_md, &files, Some(&body.slug))
        .map_err(|e| ApiError::Validation(e.to_string()))?;
    let (skill, _version) = create_skill_with_version(
        &state.pool,
        principal.owner_id(),
        &body.slug,
        &package,
        &files,
    )
    .await?;
    Ok(Json(SkillSummaryResponse::from(skill)))
}

pub async fn get(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(skill_id): Path<String>,
) -> Result<Json<SkillSummaryResponse>, ApiError> {
    let row = get_skill_for_owner(&state.pool, principal.owner_id(), &skill_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(SkillSummaryResponse::from(row)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchSkillRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
}

pub async fn patch(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(skill_id): Path<String>,
    Json(body): Json<PatchSkillRequest>,
) -> Result<Json<SkillSummaryResponse>, ApiError> {
    let row = patch_skill_metadata(
        &state.pool,
        principal.owner_id(),
        &skill_id,
        body.name.as_deref(),
        body.description.as_deref(),
        body.status.as_deref(),
    )
    .await?;
    Ok(Json(SkillSummaryResponse::from(row)))
}

pub async fn delete(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(skill_id): Path<String>,
) -> Result<(), ApiError> {
    delete_skill(&state.pool, principal.owner_id(), &skill_id).await?;
    Ok(())
}

pub async fn list_versions_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(skill_id): Path<String>,
) -> Result<Json<Vec<SkillVersionResponse>>, ApiError> {
    let rows = list_versions(&state.pool, principal.owner_id(), &skill_id).await?;
    Ok(Json(
        rows.into_iter().map(SkillVersionResponse::from).collect(),
    ))
}

pub async fn create_version(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(skill_id): Path<String>,
    Json(body): Json<SkillPackageInput>,
) -> Result<Json<SkillVersionResponse>, ApiError> {
    let skill = get_skill_for_owner(&state.pool, principal.owner_id(), &skill_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let package_files = match &body.files {
        Some(inputs) => files_from_input(inputs),
        None => {
            list_version_package_files(
                &state.pool,
                principal.owner_id(),
                &skill_id,
                skill.current_version,
            )
            .await?
        }
    };
    let package =
        SkillPackage::validate_and_build(&body.skill_md, &package_files, Some(&skill.slug))
            .map_err(|e| ApiError::Validation(e.to_string()))?;
    let version = append_skill_version(
        &state.pool,
        principal.owner_id(),
        &skill_id,
        &package,
        &package_files,
    )
    .await?;
    Ok(Json(SkillVersionResponse::from(version)))
}

pub async fn get_version(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path((skill_id, version)): Path<(String, i32)>,
) -> Result<Json<SkillVersionResponse>, ApiError> {
    let row = get_version_for_owner(&state.pool, principal.owner_id(), &skill_id, version)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(SkillVersionResponse::from(row)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachBotSkillRequest {
    pub skill_id: String,
    pub pinned_version: Option<i32>,
}

pub async fn list_bot_skills_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(bot_id): Path<String>,
) -> Result<Json<Vec<BotSkillAttachment>>, ApiError> {
    Ok(Json(
        list_bot_skills(&state.pool, principal.owner_id(), &bot_id).await?,
    ))
}

pub async fn attach_bot_skill_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(bot_id): Path<String>,
    Json(body): Json<AttachBotSkillRequest>,
) -> Result<(), ApiError> {
    attach_bot_skill(
        &state.pool,
        principal.owner_id(),
        &bot_id,
        &body.skill_id,
        body.pinned_version,
    )
    .await?;
    Ok(())
}

pub async fn detach_bot_skill_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path((bot_id, skill_id)): Path<(String, String)>,
) -> Result<(), ApiError> {
    detach_bot_skill(&state.pool, principal.owner_id(), &bot_id, &skill_id).await?;
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillDraftResponse {
    pub draft_id: String,
    pub skill_md: String,
    pub files: Vec<SkillFileInput>,
    pub parsed_name: String,
    pub parsed_description: String,
    pub draft_kind: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveRunSkillRequest {
    pub skill_md: String,
    #[serde(default)]
    pub files: Vec<SkillFileInput>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub attach_to_bot: bool,
    pub bot_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveRunSkillResponse {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub current_version: i32,
    pub attached_to_bot: bool,
}

pub async fn create_run_skill_draft(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(run_id): Path<String>,
) -> Result<Json<SkillDraftResponse>, ApiError> {
    let (package, files, kind) = generate_skill_draft_from_run(
        &state.pool,
        &state.config,
        principal.owner_id(),
        &run_id,
    )
    .await?;
    Ok(Json(SkillDraftResponse {
        draft_id: Uuid::new_v4().to_string(),
        skill_md: package.skill_md,
        parsed_name: package.frontmatter.name.clone(),
        parsed_description: package.frontmatter.description.clone(),
        draft_kind: kind.as_str().into(),
        files: files
            .into_iter()
            .map(|f| SkillFileInput {
                relative_path: f.relative_path,
                content: f.content,
                content_type: f.content_type,
            })
            .collect(),
    }))
}

pub async fn save_run_skill(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(run_id): Path<String>,
    Json(body): Json<SaveRunSkillRequest>,
) -> Result<Json<SaveRunSkillResponse>, ApiError> {
    let owner = principal.owner_id();
    let run_ok: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM agent_runs WHERE id = $1 AND owner_id = $2 AND status = 'completed')",
    )
    .bind(&run_id)
    .bind(owner)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    if !run_ok {
        return Err(ApiError::Validation(
            "Only completed runs can be saved as skills".into(),
        ));
    }
    let files = files_from_input(&body.files);
    let optional_name = body.name.as_deref().filter(|s| !s.trim().is_empty());
    let optional_description = body
        .description
        .as_deref()
        .filter(|s| !s.trim().is_empty());
    let attach_bot = if body.attach_to_bot {
        Some(body.bot_id.as_str())
    } else {
        None
    };
    let (skill, _version) = save_reviewed_skill_package(
        &state.pool,
        owner,
        &body.skill_md,
        &files,
        optional_name,
        optional_description,
        attach_bot,
    )
    .await?;
    Ok(Json(SaveRunSkillResponse {
        id: skill.id,
        slug: skill.slug,
        name: skill.name,
        description: skill.description,
        current_version: skill.current_version,
        attached_to_bot: body.attach_to_bot,
    }))
}
