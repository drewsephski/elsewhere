use crate::error::SkillPackageError;
use crate::limits::MAX_SKILL_MD_BYTES;
use crate::package::SkillPackage;

const RESPONSES_SKILL_APPENDIX_CAP: usize = 32 * 1024;

/// Bounded provider-neutral rendering for the Responses engine path.
pub fn append_skills_to_instructions(
    base: &str,
    packages: &[SkillPackage],
) -> Result<String, SkillPackageError> {
    if packages.is_empty() {
        return Ok(base.to_string());
    }
    let mut appendix = String::from("\n\n---\nElsewhere run skills (read-only reference):\n");
    for package in packages {
        appendix.push_str("\n## Skill: ");
        appendix.push_str(&package.frontmatter.name);
        appendix.push_str("\n");
        appendix.push_str(&package.frontmatter.description);
        appendix.push_str("\n\n");
        appendix.push_str(&package.skill_md);
        appendix.push_str("\n");
        if appendix.len() > RESPONSES_SKILL_APPENDIX_CAP {
            return Err(SkillPackageError::validation(
                "combined skill instructions exceed Responses appendix cap",
            ));
        }
    }
    let mut combined = base.to_string();
    if combined.len() + appendix.len() > MAX_SKILL_MD_BYTES * 4 {
        return Err(SkillPackageError::validation(
            "instructions plus skills exceed bounded Responses input",
        ));
    }
    combined.push_str(&appendix);
    Ok(combined)
}
