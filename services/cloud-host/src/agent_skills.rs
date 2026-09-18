//! Bot-facing skill tools backed by the skills service.

use agent_core::{
    AgentSkills, SkillAttachResult, SkillContext, SkillDetachResult, SkillError, SkillListEntry,
    SkillSaveDraft, SkillSaveResult,
};
use agent_skills::SkillPackageFile;
use async_trait::async_trait;
use sqlx::PgPool;

use crate::config::Config;
use crate::error::ApiError;
use crate::skills::{
    apply_skill_save_overrides, attach_bot_skill, create_skill_with_version, detach_bot_skill,
    get_skill_for_owner, list_bot_skills, list_skills, resolve_prior_completed_run,
    skill_draft_from_run,
};

pub struct PostgresAgentSkills {
    pool: PgPool,
    config: Config,
}

impl PostgresAgentSkills {
    pub fn new(pool: PgPool, config: Config) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self { pool, config })
    }
}

fn map_api(err: ApiError) -> SkillError {
    match err {
        ApiError::NotFound => SkillError::NotFound,
        ApiError::Validation(m) => SkillError::Validation(m),
        ApiError::Conflict(m) => SkillError::Conflict(m),
        other => SkillError::Internal(other.to_string()),
    }
}

#[async_trait]
impl AgentSkills for PostgresAgentSkills {
    async fn list(&self, ctx: &SkillContext) -> Result<Vec<SkillListEntry>, SkillError> {
        let skills = list_skills(&self.pool, &ctx.owner_id)
            .await
            .map_err(map_api)?;
        let attached = list_bot_skills(&self.pool, &ctx.owner_id, &ctx.bot_id)
            .await
            .map_err(map_api)?;
        Ok(skills
            .into_iter()
            .map(|row| {
                let link = attached.iter().find(|a| a.skill_id == row.id);
                SkillListEntry {
                    id: row.id,
                    slug: row.slug,
                    name: row.name,
                    description: row.description,
                    status: row.status,
                    current_version: row.current_version,
                    attached_to_bot: link.is_some(),
                    pinned_version: link.and_then(|a| a.pinned_version),
                }
            })
            .collect())
    }

    async fn prepare_save_from_recent_work(
        &self,
        ctx: &SkillContext,
        current_run_id: &str,
        optional_name: Option<&str>,
        optional_description: Option<&str>,
        attach_to_bot: bool,
    ) -> Result<SkillSaveDraft, SkillError> {
        let source_run_id = resolve_prior_completed_run(
            &self.pool,
            &ctx.owner_id,
            &ctx.bot_id,
            &ctx.source_conversation_id,
            current_run_id,
        )
        .await
        .map_err(map_api)?
        .ok_or(SkillError::NoSourceRun)?;

        let (package, files) = skill_draft_from_run(
            &self.pool,
            &self.config,
            &ctx.owner_id,
            &source_run_id,
        )
        .await
        .map_err(map_api)?;

        let (package, _files) = apply_skill_save_overrides(
            &package,
            &files,
            optional_name,
            optional_description,
        )
        .map_err(map_api)?;

        let existing = list_skills(&self.pool, &ctx.owner_id)
            .await
            .map_err(map_api)?;
        if existing
            .iter()
            .any(|row| row.slug == package.frontmatter.name && row.status == "active")
        {
            return Err(SkillError::Conflict(format!(
                "A skill named \"{}\" already exists. Choose a different name or update the existing skill on the Skills page.",
                package.frontmatter.name
            )));
        }

        Ok(SkillSaveDraft {
            source_run_id,
            slug: package.frontmatter.name.clone(),
            display_name: optional_name
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| humanize_slug(&package.frontmatter.name))
                .to_string(),
            description: package.frontmatter.description.clone(),
            skill_md: package.skill_md,
            attach_to_bot,
        })
    }

    async fn persist_save(
        &self,
        ctx: &SkillContext,
        draft: &SkillSaveDraft,
    ) -> Result<SkillSaveResult, SkillError> {
        let package = agent_skills::SkillPackage::validate_and_build(
            &draft.skill_md,
            &[] as &[SkillPackageFile],
            Some(&draft.slug),
        )
        .map_err(|e| SkillError::Validation(e.to_string()))?;

        let (skill, _version) = create_skill_with_version(
            &self.pool,
            &ctx.owner_id,
            &draft.slug,
            &package,
            &[],
        )
        .await
        .map_err(map_api)?;

        if draft.attach_to_bot {
            attach_bot_skill(&self.pool, &ctx.owner_id, &ctx.bot_id, &skill.id, None)
                .await
                .map_err(map_api)?;
        }

        let bot_name = self.bot_display_name(ctx).await?;
        Ok(SkillSaveResult {
            skill_id: skill.id,
            slug: skill.slug,
            name: skill.name,
            version: skill.current_version,
            attached_to_bot: draft.attach_to_bot,
            bot_name: bot_name.clone(),
            message: format!(
                "Skill saved as \"{}\"{}",
                skill.name,
                if draft.attach_to_bot {
                    format!(" and attached to {bot_name}")
                } else {
                    String::new()
                }
            ),
        })
    }

    async fn attach(
        &self,
        ctx: &SkillContext,
        skill_id: &str,
    ) -> Result<SkillAttachResult, SkillError> {
        let skill = get_skill_for_owner(&self.pool, &ctx.owner_id, skill_id)
            .await
            .map_err(map_api)?
            .ok_or(SkillError::NotFound)?;
        attach_bot_skill(&self.pool, &ctx.owner_id, &ctx.bot_id, skill_id, None)
            .await
            .map_err(map_api)?;
        let bot_name = self.bot_display_name(ctx).await?;
        Ok(SkillAttachResult {
            skill_id: skill.id,
            slug: skill.slug,
            name: skill.name,
            bot_name: bot_name.clone(),
            message: format!("Attached \"{}\" to {bot_name}", skill.name),
        })
    }

    async fn detach(
        &self,
        ctx: &SkillContext,
        skill_id: &str,
    ) -> Result<SkillDetachResult, SkillError> {
        let skill = get_skill_for_owner(&self.pool, &ctx.owner_id, skill_id)
            .await
            .map_err(map_api)?
            .ok_or(SkillError::NotFound)?;
        detach_bot_skill(&self.pool, &ctx.owner_id, &ctx.bot_id, skill_id)
            .await
            .map_err(map_api)?;
        let bot_name = self.bot_display_name(ctx).await?;
        Ok(SkillDetachResult {
            skill_id: skill.id,
            slug: skill.slug,
            name: skill.name,
            bot_name: bot_name.clone(),
            message: format!("Removed \"{}\" from {bot_name}", skill.name),
        })
    }

    async fn bot_display_name(&self, ctx: &SkillContext) -> Result<String, SkillError> {
        let name: Option<String> = sqlx::query_scalar(
            "SELECT name FROM bots WHERE id = $1 AND owner_id = $2",
        )
        .bind(&ctx.bot_id)
        .bind(&ctx.owner_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SkillError::Internal(e.to_string()))?;
        Ok(name.unwrap_or_else(|| "this Bot".into()))
    }
}

fn humanize_slug(slug: &str) -> String {
    slug.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
