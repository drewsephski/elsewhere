//! Focused trust/safety unit tests for GitHub coding helpers.

use cloud_host::github_coding::archive::{extract_tarball_files, MAX_COMPRESSED_TARBALL_BYTES};
use cloud_host::github_coding::check_evidence::{
    reject_forged_check_fields, verify_check_commands_from_events,
};
use serde_json::json;

#[test]
fn archive_enforces_compressed_size_limit() {
    let huge = vec![0u8; MAX_COMPRESSED_TARBALL_BYTES + 1];
    let err = extract_tarball_files(&huge).expect_err("limit");
    assert!(err.message().contains("maximum download"));
}

#[test]
fn forged_model_checks_rejected() {
    let err = reject_forged_check_fields(&json!({ "checks": [] })).expect_err("forged");
    assert!(err.message().contains("checks"));
}

#[test]
fn workspace_exec_evidence_required() {
    let err = verify_check_commands_from_events(&[], &[String::from("pnpm test")]).expect_err("missing");
    assert!(err.message().contains("not executed"));
}
