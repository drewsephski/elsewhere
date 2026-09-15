use sha2::{Digest, Sha256};

use crate::error::SkillPackageError;
use crate::limits::{
    MAX_FILE_BYTES, MAX_PACKAGE_FILES, MAX_RELATIVE_PATH_LEN, MAX_SKILL_MD_BYTES,
    MAX_TOTAL_PACKAGE_BYTES,
};
use crate::parse::{parse_skill_md, slug_from_name, ValidatedSkillFrontmatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillPackageFile {
    pub relative_path: String,
    pub content: String,
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillPackage {
    pub frontmatter: ValidatedSkillFrontmatter,
    pub skill_md_body: String,
    pub skill_md: String,
    pub files: Vec<SkillPackageFile>,
    pub content_hash: String,
}

impl SkillPackage {
    pub fn validate_and_build(
        skill_md: &str,
        extra_files: &[SkillPackageFile],
        expected_slug: Option<&str>,
    ) -> Result<Self, SkillPackageError> {
        if skill_md.len() > MAX_SKILL_MD_BYTES {
            return Err(SkillPackageError::validation(format!(
                "SKILL.md exceeds {MAX_SKILL_MD_BYTES} bytes"
            )));
        }

        let (frontmatter, body) = parse_skill_md(skill_md)?;
        if let Some(slug) = expected_slug {
            if slug != frontmatter.name {
                return Err(SkillPackageError::validation(
                    "SKILL.md name must match the skill slug",
                ));
            }
        }
        slug_from_name(&frontmatter.name)?;

        if extra_files.len() > MAX_PACKAGE_FILES {
            return Err(SkillPackageError::validation(format!(
                "skill package exceeds {MAX_PACKAGE_FILES} files"
            )));
        }

        let mut total_bytes = skill_md.len();
        let mut normalized_files = Vec::with_capacity(extra_files.len());
        for file in extra_files {
            validate_relative_path(&file.relative_path)?;
            if file.relative_path == "SKILL.md" {
                return Err(SkillPackageError::validation(
                    "duplicate SKILL.md must not appear in files list",
                ));
            }
            if file.content.len() > MAX_FILE_BYTES {
                return Err(SkillPackageError::validation(format!(
                    "file {} exceeds {MAX_FILE_BYTES} bytes",
                    file.relative_path
                )));
            }
            total_bytes += file.content.len();
            if total_bytes > MAX_TOTAL_PACKAGE_BYTES {
                return Err(SkillPackageError::validation(format!(
                    "skill package exceeds {MAX_TOTAL_PACKAGE_BYTES} total bytes"
                )));
            }
            normalized_files.push(file.clone());
        }

        let content_hash = compute_content_hash(skill_md, &normalized_files);

        Ok(Self {
            frontmatter,
            skill_md_body: body,
            skill_md: skill_md.to_string(),
            files: normalized_files,
            content_hash,
        })
    }
}

pub fn validate_relative_path(path: &str) -> Result<(), SkillPackageError> {
    if path.is_empty() || path.len() > MAX_RELATIVE_PATH_LEN {
        return Err(SkillPackageError::validation(
            "relative path length is invalid",
        ));
    }
    if path.starts_with('/') || path.starts_with('\\') {
        return Err(SkillPackageError::validation(
            "relative paths must not be absolute",
        ));
    }
    if path.contains('\\') {
        return Err(SkillPackageError::validation(
            "relative paths must use forward slashes",
        ));
    }
    if path.contains("..") {
        return Err(SkillPackageError::validation(
            "relative paths must not contain ..",
        ));
    }
    for segment in path.split('/') {
        if segment.is_empty() {
            return Err(SkillPackageError::validation(
                "relative paths must not contain empty segments",
            ));
        }
        if segment == "." {
            return Err(SkillPackageError::validation(
                "relative paths must not contain . segments",
            ));
        }
    }
    Ok(())
}

fn compute_content_hash(skill_md: &str, files: &[SkillPackageFile]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(skill_md.as_bytes());
    let mut paths: Vec<_> = files.iter().map(|f| f.relative_path.as_str()).collect();
    paths.sort_unstable();
    for path in paths {
        if let Some(file) = files.iter().find(|f| f.relative_path == path) {
            hasher.update(path.as_bytes());
            hasher.update(file.content.as_bytes());
        }
    }
    format!("{:x}", hasher.finalize())
}
