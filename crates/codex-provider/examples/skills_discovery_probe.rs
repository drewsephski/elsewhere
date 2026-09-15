//! Codex native skill discovery compatibility: materialize `.agents/skills` under run cwd.

use agent_skills::{materialize_agents_skills, NativeSkillsLayout, SkillPackage};
use std::path::Path;

fn sample_package() -> SkillPackage {
    let md = "---\nname: probe-skill\ndescription: probe\n---\n\nProbe body.\n";
    SkillPackage::validate_and_build(md, &[], None).expect("valid package")
}

fn main() {
    let cwd = tempfile::tempdir().expect("tempdir");
    let package = sample_package();
    materialize_agents_skills(cwd.path(), &[package]).expect("materialize");
    let skill_md = NativeSkillsLayout::agents_skills_root(cwd.path())
        .join("probe-skill")
        .join("SKILL.md");
    assert!(
        skill_md.is_file(),
        "expected {} to exist for Codex repo-scoped discovery",
        skill_md.display()
    );
    let rel = agent_skills::native_skills_relative_dir("probe-skill");
    assert!(Path::new(&rel).ends_with("probe-skill"));
    println!(
        "Skill discovery probe passed: materialized {} under cwd",
        rel
    );
}
