//! Exact Git tree reconciliation for owner-approved publish revisions.

use crate::connectors::github_client::GitHubTreeChange;
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeBlobEntry {
    pub mode: String,
    pub sha: String,
}

pub type TreeBlobMap = BTreeMap<String, TreeBlobEntry>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeReconcileError {
    TruncatedTree,
    InvalidTreeResponse,
}

/// Git blob object name (SHA-1), compatible with GitHub's blob SHAs.
pub fn git_blob_object_sha(content: &[u8]) -> String {
    let header = format!("blob {}\0", content.len());
    let mut hasher = Sha1::new();
    hasher.update(header.as_bytes());
    hasher.update(content);
    hex::encode(hasher.finalize())
}

pub fn blob_map_from_recursive_tree(tree: &Value) -> Result<TreeBlobMap, TreeReconcileError> {
    if tree.get("truncated").and_then(|v| v.as_bool()) == Some(true) {
        return Err(TreeReconcileError::TruncatedTree);
    }
    let entries = tree
        .get("tree")
        .and_then(|v| v.as_array())
        .ok_or(TreeReconcileError::InvalidTreeResponse)?;
    let mut map = TreeBlobMap::new();
    for entry in entries {
        if entry.get("type").and_then(|t| t.as_str()) != Some("blob") {
            continue;
        }
        let path = entry
            .get("path")
            .and_then(|p| p.as_str())
            .ok_or(TreeReconcileError::InvalidTreeResponse)?;
        let mode = entry
            .get("mode")
            .and_then(|m| m.as_str())
            .ok_or(TreeReconcileError::InvalidTreeResponse)?
            .to_string();
        let sha = entry
            .get("sha")
            .and_then(|s| s.as_str())
            .ok_or(TreeReconcileError::InvalidTreeResponse)?
            .to_string();
        map.insert(path.to_string(), TreeBlobEntry { mode, sha });
    }
    Ok(map)
}

pub fn expected_tree_after_changes(
    parent: &TreeBlobMap,
    changes: &[GitHubTreeChange],
) -> Result<TreeBlobMap, String> {
    let mut expected = parent.clone();
    for change in changes {
        if change.deleted {
            expected.remove(&change.path);
            continue;
        }
        let content = change
            .content
            .as_ref()
            .ok_or_else(|| format!("missing content for {}", change.path))?;
        let sha = git_blob_object_sha(content);
        expected.insert(
            change.path.clone(),
            TreeBlobEntry {
                mode: change.mode.clone(),
                sha,
            },
        );
    }
    Ok(expected)
}

pub fn trees_match(expected: &TreeBlobMap, candidate: &TreeBlobMap) -> bool {
    expected == candidate
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn change(path: &str, content: &str, mode: &str) -> GitHubTreeChange {
        GitHubTreeChange {
            path: path.into(),
            mode: mode.into(),
            deleted: false,
            content: Some(content.as_bytes().to_vec()),
        }
    }

    fn delete(path: &str) -> GitHubTreeChange {
        GitHubTreeChange {
            path: path.into(),
            mode: "100644".into(),
            deleted: true,
            content: None,
        }
    }

    #[test]
    fn exact_prepared_tree_matches() {
        let parent = TreeBlobMap::from([(
            "README.md".into(),
            TreeBlobEntry {
                mode: "100644".into(),
                sha: git_blob_object_sha(b"Hello"),
            },
        )]);
        let changes = vec![change("README.md", "Hello from Elsewhere", "100644")];
        let expected = expected_tree_after_changes(&parent, &changes).unwrap();
        assert_eq!(expected.len(), 1);
        assert!(trees_match(&expected, &expected));
    }

    #[test]
    fn rejects_extra_modified_path() {
        let parent = TreeBlobMap::from([(
            "README.md".into(),
            TreeBlobEntry {
                mode: "100644".into(),
                sha: git_blob_object_sha(b"Hello"),
            },
        )]);
        let changes = vec![change("README.md", "Revised", "100644")];
        let expected = expected_tree_after_changes(&parent, &changes).unwrap();
        let mut candidate = expected.clone();
        candidate.insert(
            "src/auth.ts".into(),
            TreeBlobEntry {
                mode: "100644".into(),
                sha: "deadbeef".into(),
            },
        );
        assert!(!trees_match(&expected, &candidate));
    }

    #[test]
    fn rejects_extra_added_path() {
        let parent = TreeBlobMap::new();
        let changes = vec![change("README.md", "Only", "100644")];
        let expected = expected_tree_after_changes(&parent, &changes).unwrap();
        let mut candidate = expected.clone();
        candidate.insert(
            "extra.txt".into(),
            TreeBlobEntry {
                mode: "100644".into(),
                sha: git_blob_object_sha(b"x"),
            },
        );
        assert!(!trees_match(&expected, &candidate));
    }

    #[test]
    fn rejects_missing_expected_change() {
        let parent = TreeBlobMap::from([(
            "README.md".into(),
            TreeBlobEntry {
                mode: "100644".into(),
                sha: git_blob_object_sha(b"Hello"),
            },
        )]);
        let changes = vec![change("README.md", "Revised", "100644")];
        let expected = expected_tree_after_changes(&parent, &changes).unwrap();
        let candidate = parent;
        assert!(!trees_match(&expected, &candidate));
    }

    #[test]
    fn rejects_wrong_executable_mode() {
        let parent = TreeBlobMap::new();
        let changes = vec![change("run.sh", "echo hi", "100755")];
        let expected = expected_tree_after_changes(&parent, &changes).unwrap();
        let mut candidate = expected.clone();
        if let Some(entry) = candidate.get_mut("run.sh") {
            entry.mode = "100644".into();
        }
        assert!(!trees_match(&expected, &candidate));
    }

    #[test]
    fn expected_deletion_removes_path() {
        let parent = TreeBlobMap::from([
            (
                "README.md".into(),
                TreeBlobEntry {
                    mode: "100644".into(),
                    sha: git_blob_object_sha(b"Hello"),
                },
            ),
            (
                "old.txt".into(),
                TreeBlobEntry {
                    mode: "100644".into(),
                    sha: git_blob_object_sha(b"gone"),
                },
            ),
        ]);
        let changes = vec![delete("old.txt")];
        let expected = expected_tree_after_changes(&parent, &changes).unwrap();
        assert!(!expected.contains_key("old.txt"));
        assert!(expected.contains_key("README.md"));
    }

    #[test]
    fn truncated_tree_response_errors() {
        let tree = json!({ "truncated": true, "tree": [] });
        assert_eq!(
            blob_map_from_recursive_tree(&tree),
            Err(TreeReconcileError::TruncatedTree)
        );
    }
}
