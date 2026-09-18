//! Draft Agent Skill packages from completed runs (review before save).

use sqlx::{PgPool, Row};

use agent_skills::{SkillPackage, SkillPackageFile};

use crate::error::ApiError;
use crate::provider_profile;
use crate::run_engine_select::RunEngineMode;
use codex_provider::run_toolless_codex_turn;

const DRAFT_DEVELOPER: &str = r#"You generate DRAFT Open Agent Skill packages for Elsewhere.

Return ONLY valid JSON with keys:
- skillMd: full SKILL.md including YAML frontmatter (name, description) and body instructions
- files: optional array of { relativePath, content, contentType? }

Rules:
- name must be lowercase hyphenated slug matching frontmatter name
- description must be concise
- Do not include executable host scripts; references/ and assets/ only
- Keep total content bounded and practical
"#;

pub async fn generate_skill_draft_from_run(
    pool: &PgPool,
    config: &crate::config::Config,
    owner: &str,
    run_id: &str,
) -> Result<(SkillPackage, Vec<SkillPackageFile>, SkillDraftKind), ApiError> {
    let (task, answer) = load_completed_run_content(pool, owner, run_id).await?;

    let user_prompt = format!(
        "Original task:\n{task}\n\nFinal answer:\n{answer}\n\nProduce a reusable Agent Skill draft that captures the repeatable process (when to use, steps, validation, and result format), not just the final answer as a prompt.",
    );

    let profile = if config.run_engine == RunEngineMode::Responses {
        None
    } else {
        provider_profile::profile_for_owner(pool, config, owner).await?
    };

    let codex_attempt = run_toolless_codex_turn(
        config.codex_executable.clone(),
        profile,
        &agent_core::DEFAULT_MODEL,
        DRAFT_DEVELOPER,
        &user_prompt,
    )
    .await;

    if let Ok(raw) = codex_attempt {
        if let Ok(parsed) = parse_skill_draft_json(&raw) {
            return Ok((parsed.0, parsed.1, SkillDraftKind::Generated));
        }
    }

    if config.allow_skill_draft_heuristic {
        let (package, files) = heuristic_skill_draft(&task, &answer)?;
        return Ok((package, files, SkillDraftKind::Heuristic));
    }

    Err(ApiError::Validation(
        "Could not generate a skill draft from this run. Try again when draft generation is available, or edit the skill manually on the Skills page.".into(),
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillDraftKind {
    Generated,
    Heuristic,
}

impl SkillDraftKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Generated => "generated",
            Self::Heuristic => "heuristic",
        }
    }
}

/// Prefer Codex when available; otherwise build a deterministic draft (tests, Responses-only hosts).
pub async fn skill_draft_from_run(
    pool: &PgPool,
    config: &crate::config::Config,
    owner: &str,
    run_id: &str,
) -> Result<(SkillPackage, Vec<SkillPackageFile>, SkillDraftKind), ApiError> {
    generate_skill_draft_from_run(pool, config, owner, run_id).await
}

pub async fn resolve_prior_completed_run(
    pool: &PgPool,
    owner: &str,
    bot_id: &str,
    conversation_id: &str,
    exclude_run_id: &str,
) -> Result<Option<String>, ApiError> {
    let row: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT r.id
        FROM agent_runs r
        WHERE r.owner_id = $1
          AND r.bot_id = $2
          AND r.conversation_id = $3
          AND r.status = 'completed'
          AND r.id <> $4
          AND r.assistant_message_id IS NOT NULL
        ORDER BY COALESCE(r.finished_at, r.updated_at) DESC, r.created_at DESC
        LIMIT 1
        "#,
    )
    .bind(owner)
    .bind(bot_id)
    .bind(conversation_id)
    .bind(exclude_run_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(row.map(|(id,)| id))
}

pub fn apply_skill_save_overrides(
    package: &SkillPackage,
    files: &[SkillPackageFile],
    optional_name: Option<&str>,
    optional_description: Option<&str>,
) -> Result<(SkillPackage, Vec<SkillPackageFile>), ApiError> {
    let slug = optional_name
        .map(title_to_skill_slug)
        .transpose()
        .map_err(|e| ApiError::Validation(e.to_string()))?
        .unwrap_or_else(|| package.frontmatter.name.clone());
    let description = optional_description
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(package.frontmatter.description.as_str())
        .to_string();
    let body = package.skill_md_body.trim();
    let skill_md = format!(
        "---\nname: {slug}\ndescription: {description}\n---\n\n{body}\n"
    );
    let rebuilt =
        SkillPackage::validate_and_build(&skill_md, files, Some(&slug))
            .map_err(|e| ApiError::Validation(e.to_string()))?;
    Ok((rebuilt, files.to_vec()))
}

async fn load_completed_run_content(
    pool: &PgPool,
    owner: &str,
    run_id: &str,
) -> Result<(String, String), ApiError> {
    let row = sqlx::query(
        r#"
        SELECT r.status, q.user_message,
               (SELECT body FROM messages m WHERE m.id = r.assistant_message_id) AS assistant_body
        FROM agent_runs r
        LEFT JOIN work_queue q ON q.run_id = r.id
        WHERE r.id = $1 AND r.owner_id = $2
        "#,
    )
    .bind(run_id)
    .bind(owner)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or(ApiError::NotFound)?;

    if row.get::<String, _>("status") != "completed" {
        return Err(ApiError::Validation(
            "Only completed runs can be saved as skills".into(),
        ));
    }

    let task: String = row
        .get::<Option<String>, _>("user_message")
        .unwrap_or_default();
    let answer: String = row
        .get::<Option<String>, _>("assistant_body")
        .unwrap_or_default();
    Ok((task, answer))
}

fn parse_skill_draft_json(
    raw: &str,
) -> Result<(SkillPackage, Vec<SkillPackageFile>), ApiError> {
    let value: serde_json::Value = serde_json::from_str(raw.trim())
        .map_err(|_| ApiError::Internal("skill draft model returned invalid JSON".into()))?;
    let skill_md = value
        .get("skillMd")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::Internal("skill draft missing skillMd".into()))?;
    let mut files = Vec::new();
    if let Some(list) = value.get("files").and_then(|v| v.as_array()) {
        for item in list {
            files.push(SkillPackageFile {
                relative_path: item
                    .get("relativePath")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                content: item
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                content_type: item
                    .get("contentType")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            });
        }
    }
    let package = SkillPackage::validate_and_build(skill_md, &files, None)
        .map_err(|e| ApiError::Validation(e.to_string()))?;
    Ok((package, files))
}

fn heuristic_skill_draft(
    task: &str,
    answer: &str,
) -> Result<(SkillPackage, Vec<SkillPackageFile>), ApiError> {
    let slug = title_to_skill_slug(task).unwrap_or_else(|_| "saved-workflow".into());
    let description = truncate_chars(
        task.trim(),
        160,
        "Reusable workflow saved from a completed assignment.",
    );
    let skill_md = format!(
        "---\nname: {slug}\ndescription: {description}\n---\n\n## When to use\nUse when the owner asks for work similar to the completed assignment below.\n\n## Inputs and access\n- Clarify any missing constraints before starting.\n- Use the Bot's normal tools; request approval before mutations.\n\n## Work sequence\n1. Restate the goal in your own words.\n2. Execute the same investigative and delivery steps you used successfully.\n3. Validate outputs against the acceptance criteria.\n4. Summarize results and point to saved artifacts.\n\n## Validation\n- Confirm the deliverable matches the original request.\n- Do not claim files or pages exist until tool results succeed.\n\n## Result format\nProvide a concise summary plus any saved file names or links.\n\n## Reference assignment\n**Task:** {task}\n\n**Successful outcome (reference only):**\n{answer}\n"
    );
    let files = Vec::new();
    let package = SkillPackage::validate_and_build(&skill_md, &files, Some(&slug))
        .map_err(|e| ApiError::Validation(e.to_string()))?;
    Ok((package, files))
}

pub fn title_to_skill_slug(title: &str) -> Result<String, ApiError> {
    let mut slug = String::new();
    let mut last_hyphen = false;
    for ch in title.trim().to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_hyphen = false;
        } else if !last_hyphen && !slug.is_empty() {
            slug.push('-');
            last_hyphen = true;
        }
        if slug.len() >= 64 {
            break;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        slug.push_str("saved-workflow");
    }
    let probe = format!("---\nname: {slug}\ndescription: probe\n---\n\n");
    SkillPackage::validate_and_build(&probe, &[], Some(&slug))
        .map_err(|e| ApiError::Validation(e.to_string()))?;
    Ok(slug)
}

fn truncate_chars(input: &str, max: usize, fallback: &str) -> String {
    if input.is_empty() {
        return fallback.to_string();
    }
    if input.chars().count() <= max {
        return input.to_string();
    }
    format!(
        "{}…",
        input.chars().take(max.saturating_sub(1)).collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::title_to_skill_slug;

    #[test]
    fn title_to_skill_slug_normalizes_display_titles() {
        assert_eq!(
            title_to_skill_slug("Competitor Brief").expect("slug"),
            "competitor-brief"
        );
    }
}

