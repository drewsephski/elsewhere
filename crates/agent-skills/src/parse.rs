use regex::Regex;
use std::sync::LazyLock;

use crate::error::SkillPackageError;
use crate::limits::MAX_SKILL_NAME_LEN;

static SKILL_NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9]+(-[a-z0-9]+)*$").expect("skill name regex"));

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSkillFrontmatter {
    pub name: String,
    pub description: String,
}

pub fn parse_skill_md(content: &str) -> Result<(ValidatedSkillFrontmatter, String), SkillPackageError> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return Err(SkillPackageError::validation(
            "SKILL.md must begin with YAML frontmatter delimited by ---",
        ));
    }
    let rest = trimmed.strip_prefix("---").unwrap_or(trimmed);
    let rest = rest.trim_start_matches(['\n', '\r']);
    let end = rest
        .find("\n---")
        .ok_or_else(|| SkillPackageError::validation("SKILL.md frontmatter must end with ---"))?;
    let yaml = &rest[..end];
    let body = rest[end + 4..].trim_start_matches(['\n', '\r']);

    #[derive(serde::Deserialize)]
    struct RawFrontmatter {
        name: Option<String>,
        description: Option<String>,
    }

    let raw: RawFrontmatter = serde_yaml::from_str(yaml)
        .map_err(|e| SkillPackageError::validation(format!("invalid SKILL.md frontmatter: {e}")))?;

    let name = raw
        .name
        .unwrap_or_default()
        .trim()
        .to_string();
    let description = raw
        .description
        .unwrap_or_default()
        .trim()
        .to_string();

    if name.is_empty() {
        return Err(SkillPackageError::validation(
            "SKILL.md frontmatter requires name",
        ));
    }
    if description.is_empty() {
        return Err(SkillPackageError::validation(
            "SKILL.md frontmatter requires description",
        ));
    }
    validate_skill_name(&name)?;

    Ok((
        ValidatedSkillFrontmatter { name, description },
        body.to_string(),
    ))
}

pub fn validate_skill_name(name: &str) -> Result<(), SkillPackageError> {
    if name.is_empty() || name.len() > MAX_SKILL_NAME_LEN {
        return Err(SkillPackageError::validation(format!(
            "skill name must be 1 to {MAX_SKILL_NAME_LEN} characters"
        )));
    }
    if !SKILL_NAME_RE.is_match(name) {
        return Err(SkillPackageError::validation(
            "skill name must use lowercase letters, digits, and single hyphens (no leading/trailing hyphen)",
        ));
    }
    Ok(())
}

pub fn slug_from_name(name: &str) -> Result<String, SkillPackageError> {
    validate_skill_name(name)?;
    Ok(name.to_string())
}
