use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::workspace_git::GitFileChange;

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
