use std::collections::HashMap;

pub const BRANCH_PREFIX: &str = "elsewhere/";

pub fn is_elsewhere_managed_branch(branch: &str) -> bool {
    branch.starts_with(BRANCH_PREFIX)
}

pub fn checkout_root(owner: &str, repo: &str, run_id: &str) -> String {
    let suffix = run_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(16)
        .collect::<String>()
        .to_ascii_lowercase();
    let suffix = if suffix.is_empty() { "run" } else { suffix.as_str() };
    format!("/workspace/repos/{}/{}/{}", owner, repo, suffix)
}

#[allow(dead_code)]
pub fn staging_zip_path(run_id: &str) -> String {
    format!("/workspace/.elsewhere/github/{}/source.zip", run_id)
}

pub fn sanitize_task_slug(raw: &str) -> Result<String, String> {
    let slug = raw.trim().to_ascii_lowercase();
    if slug.is_empty() {
        return Ok("task".into());
    }
    let mut out = String::new();
    for ch in slug.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' {
            out.push(ch);
        } else if ch.is_whitespace() || ch == '_' {
            if !out.ends_with('-') && !out.is_empty() {
                out.push('-');
            }
        }
    }
    let mut out = out.trim_matches('-').to_string();
    if out.is_empty() {
        return Err("task slug must contain letters or numbers".into());
    }
    if out.len() > 48 {
        out.truncate(48);
        while out.ends_with('-') {
            out.pop();
        }
    }
    Ok(out)
}

pub fn working_branch(slug: &str, run_id: &str) -> String {
    let slug = sanitize_task_slug(slug).unwrap_or_else(|_| "task".into());
    let suffix = run_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(8)
        .collect::<String>()
        .to_ascii_lowercase();
    let suffix = if suffix.is_empty() { "run" } else { suffix.as_str() };
    format!("{BRANCH_PREFIX}{slug}-{suffix}")
}

pub fn path_within_checkout(checkout: &str, relative: &str) -> Result<String, String> {
    let relative = relative.trim_start_matches("./");
    if relative.contains("..") {
        return Err("path escapes repository checkout".into());
    }
    let full = format!("{checkout}/{relative}");
    if !full.starts_with(checkout) {
        return Err("path escapes repository checkout".into());
    }
    Ok(full)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub path: String,
    pub previous: Option<Vec<u8>>,
    pub current: Option<Vec<u8>>,
}

pub fn diff_against_baseline(
    baseline: &HashMap<String, Vec<u8>>,
    current: &HashMap<String, Vec<u8>>,
) -> Vec<FileChange> {
    let mut paths: Vec<String> = baseline.keys().chain(current.keys()).cloned().collect();
    paths.sort();
    paths.dedup();
    let mut changes = Vec::new();
    for path in paths {
        let before = baseline.get(&path);
        let after = current.get(&path);
        if before == after {
            continue;
        }
        changes.push(FileChange {
            path,
            previous: before.cloned(),
            current: after.cloned(),
        });
    }
    changes
}

pub fn summarize_changes(changes: &[FileChange]) -> Vec<String> {
    changes.iter().map(|c| c.path.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_sanitization_and_branch() {
        assert_eq!(sanitize_task_slug("Fix README").unwrap(), "fix-readme");
        assert_eq!(
            working_branch("Fix README", "run-abc12345"),
            "elsewhere/fix-readme-runabc12"
        );
    }

    #[test]
    fn rejects_path_escape() {
        assert!(path_within_checkout("/workspace/repos/o/r", "../secret").is_err());
    }

    #[test]
    fn checkout_paths_are_run_scoped() {
        let a = checkout_root("acme", "demo", "run-parallel-a");
        let b = checkout_root("acme", "demo", "run-parallel-b");
        assert_ne!(a, b);
        assert!(a.contains("/acme/demo/"));
    }

    #[test]
    fn diff_detects_modified_file() {
        let mut baseline = HashMap::new();
        baseline.insert("README.md".into(), b"Hello".to_vec());
        let mut current = baseline.clone();
        current.insert("README.md".into(), b"Hello from Elsewhere".to_vec());
        let changes = diff_against_baseline(&baseline, &current);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "README.md");
    }
}
