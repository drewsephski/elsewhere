use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use agent_core::GithubCodingError;

use super::workspace_git::GitFileChange;

pub const MAX_PUBLISH_CHANGED_FILES: usize = 1_000;
pub const MAX_PUBLISH_FILE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_PUBLISH_TOTAL_BYTES: usize = 32 * 1024 * 1024;

const PUBLISH_TOO_LARGE_MSG: &str =
    "This change set is too large to publish through Elsewhere in one pull request.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedPublish {
    pub local_baseline_commit_sha: String,
    pub changes: Vec<GitFileChange>,
    pub fingerprint: String,
}

impl PreparedPublish {
    pub fn from_changes(baseline_sha: &str, changes: Vec<GitFileChange>) -> Self {
        let fingerprint = fingerprint_changes(baseline_sha, &changes);
        Self {
            local_baseline_commit_sha: baseline_sha.to_string(),
            changes,
            fingerprint,
        }
    }
}

pub fn validate_prepared_publish_limits(changes: &[GitFileChange]) -> Result<(), GithubCodingError> {
    if changes.len() > MAX_PUBLISH_CHANGED_FILES {
        return Err(GithubCodingError::Validation(PUBLISH_TOO_LARGE_MSG.into()));
    }
    let mut total_bytes = 0usize;
    for change in changes {
        if let Some(bytes) = &change.bytes {
            if bytes.len() > MAX_PUBLISH_FILE_BYTES {
                return Err(GithubCodingError::Validation(PUBLISH_TOO_LARGE_MSG.into()));
            }
            total_bytes += bytes.len();
            if total_bytes > MAX_PUBLISH_TOTAL_BYTES {
                return Err(GithubCodingError::Validation(PUBLISH_TOO_LARGE_MSG.into()));
            }
        }
    }
    Ok(())
}

pub fn fingerprint_changes(baseline_sha: &str, changes: &[GitFileChange]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(baseline_sha.as_bytes());
    for change in changes {
        hasher.update(change.path.as_bytes());
        hasher.update(change.mode.as_bytes());
        hasher.update([u8::from(change.deleted)]);
        if let Some(bytes) = &change.bytes {
            hasher.update(bytes);
        }
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change_with_bytes(path: &str, len: usize) -> GitFileChange {
        GitFileChange {
            path: path.to_string(),
            mode: "100644".into(),
            deleted: false,
            bytes: Some(vec![0u8; len]),
        }
    }

    #[test]
    fn normal_source_change_within_limits() {
        let changes = vec![change_with_bytes("src/main.rs", 1024)];
        validate_prepared_publish_limits(&changes).expect("ok");
    }

    #[test]
    fn rejects_per_file_limit() {
        let changes = vec![change_with_bytes("big.bin", MAX_PUBLISH_FILE_BYTES + 1)];
        let err = validate_prepared_publish_limits(&changes).expect_err("big file");
        assert!(err.message().contains("too large"));
    }

    #[test]
    fn rejects_total_byte_limit() {
        let chunk = MAX_PUBLISH_TOTAL_BYTES / 2 + 1;
        let changes = vec![
            change_with_bytes("a.ts", chunk),
            change_with_bytes("b.ts", chunk),
        ];
        validate_prepared_publish_limits(&changes).expect_err("total");
    }

    #[test]
    fn rejects_path_count_limit() {
        let changes = (0..MAX_PUBLISH_CHANGED_FILES + 1)
            .map(|i| GitFileChange {
                path: format!("f{i}.ts"),
                mode: "100644".into(),
                deleted: true,
                bytes: None,
            })
            .collect::<Vec<_>>();
        validate_prepared_publish_limits(&changes).expect_err("count");
    }
}
