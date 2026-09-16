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
) -> Result<(SkillPackage, Vec<SkillPackageFile>), ApiError> {
    let row = sqlx::query(
        r#"
        SELECT r.bot_id, r.status, q.user_message,
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
    let answer: Option<String> = row.get("assistant_body");

    let user_prompt = format!(
        "Original task:\n{task}\n\nFinal answer:\n{}\n\nProduce a reusable Agent Skill draft.",
        answer.unwrap_or_default()
    );

    let profile = if config.run_engine == RunEngineMode::Responses {
        None
    } else {
        provider_profile::profile_for_owner(pool, config, owner).await?
    };

    let raw = run_toolless_codex_turn(
        config.codex_executable.clone(),
        profile,
        &agent_core::DEFAULT_MODEL,
        DRAFT_DEVELOPER,
        &user_prompt,
    )
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let value: serde_json::Value = serde_json::from_str(raw.trim())
        .map_err(|_| ApiError::Internal("skill draft model returned invalid JSON".into()))?;
    let skill_md = value
        .get("skillMd")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::Internal("skill draft missing skillMd".into()))?;
    let mut files = Vec::new();
    if let Some(list) = value.get("files").and_then(|v| v.as_array()) {
        for item in list {
            let relative_path = item
                .get("relativePath")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let content = item
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let content_type = item
                .get("contentType")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            files.push(SkillPackageFile {
                relative_path,
                content,
                content_type,
            });
        }
    }
    let package = SkillPackage::validate_and_build(skill_md, &files, None)
        .map_err(|e| ApiError::Validation(e.to_string()))?;
    Ok((package, files))
}
