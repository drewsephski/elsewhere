use crate::package::{validate_relative_path, SkillPackage, SkillPackageFile};
use crate::parse::{parse_skill_md, validate_skill_name};

#[test]
fn valid_skill_md_parses() {
    let md = "---\nname: competitor-research\ndescription: Research competitors\n---\n\nDo work.\n";
    let (fm, body) = parse_skill_md(md).expect("parse");
    assert_eq!(fm.name, "competitor-research");
    assert_eq!(body.trim(), "Do work.");
}

#[test]
fn missing_frontmatter_rejected() {
    assert!(parse_skill_md("# no frontmatter").is_err());
}

#[test]
fn invalid_name_rejected() {
    assert!(validate_skill_name("Bad_Name").is_err());
}

#[test]
fn unicode_path_allowed() {
    validate_relative_path("references/日本語.md").expect("unicode segment");
}

#[test]
fn dot_dot_path_rejected() {
    assert!(validate_relative_path("references/../secret").is_err());
}

#[test]
fn absolute_path_rejected() {
    assert!(validate_relative_path("/etc/passwd").is_err());
}

#[test]
fn package_limits_enforced() {
    let md = "---\nname: tiny\ndescription: d\n---\n";
    let huge = "x".repeat(600_000);
    let err = SkillPackage::validate_and_build(md, &[SkillPackageFile {
        relative_path: "references/big.md".into(),
        content: huge,
        content_type: None,
    }], None);
    assert!(err.is_err());
}

#[test]
fn content_hash_stable() {
    let md = "---\nname: a\ndescription: b\n---\n";
    let p1 = SkillPackage::validate_and_build(md, &[], None).expect("p1");
    let p2 = SkillPackage::validate_and_build(md, &[], None).expect("p2");
    assert_eq!(p1.content_hash, p2.content_hash);
}
