use std::path::{Path, PathBuf};

use crate::error::SkillPackageError;
use crate::package::SkillPackage;

/// Directory layout Codex discovers when cwd is the run temp directory.
pub struct NativeSkillsLayout;

impl NativeSkillsLayout {
    pub fn agents_skills_root(cwd: &Path) -> PathBuf {
        cwd.join(".agents").join("skills")
    }
}

pub fn native_skills_relative_dir(skill_name: &str) -> String {
    format!(".agents/skills/{skill_name}")
}

/// Write validated packages under `{cwd}/.agents/skills/{name}/`.
/// Scripts and assets are stored as files only; nothing is marked executable on the host.
pub fn materialize_agents_skills(
    cwd: &Path,
    packages: &[SkillPackage],
) -> Result<(), SkillPackageError> {
    let root = NativeSkillsLayout::agents_skills_root(cwd);
    for package in packages {
        let skill_dir = root.join(&package.frontmatter.name);
        std::fs::create_dir_all(&skill_dir)?;
        std::fs::write(skill_dir.join("SKILL.md"), &package.skill_md)?;
        for file in &package.files {
            let dest = skill_dir.join(&file.relative_path);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(dest, &file.content)?;
        }
    }
    Ok(())
}
