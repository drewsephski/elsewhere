//! Workspace listing rules shared by guest computers and the cloud API.

/// Reserved Elsewhere housekeeping paths (not user-facing deliverables).
pub fn is_internal_workspace_entry(name: &str) -> bool {
    name.starts_with(".elsewhere-")
}

pub fn filter_workspace_listing(
    entries: impl IntoIterator<Item = crate::computer::WorkspaceEntry>,
) -> Vec<crate::computer::WorkspaceEntry> {
    entries
        .into_iter()
        .filter(|entry| !is_internal_workspace_entry(&entry.name))
        .collect()
}

/// Absolute path under `/workspace` (listing root allowed).
pub fn normalize_workspace_path(path: &str) -> Result<String, &'static str> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("path is required");
    }
    if !trimmed.starts_with('/') {
        return Err("path must be absolute");
    }
    if trimmed.contains("..") {
        return Err("path must not contain ..");
    }
    if trimmed != "/workspace" && !trimmed.starts_with("/workspace/") {
        return Err("path must be under /workspace");
    }
    Ok(trimmed.to_string())
}

/// User-visible read path (listing root allowed; reserved markers rejected).
pub fn validate_workspace_readable_path(path: &str) -> Result<String, &'static str> {
    let path = normalize_workspace_path(path)?;
    if path != "/workspace" {
        let name = path.rsplit('/').next().unwrap_or("");
        if name.is_empty() || is_internal_workspace_entry(name) {
            return Err("cannot read this path");
        }
    }
    Ok(path)
}

/// User-visible file or folder (not `/workspace` root or Elsewhere markers).
pub fn validate_workspace_mutation_path(path: &str) -> Result<String, &'static str> {
    let path = normalize_workspace_path(path)?;
    if path == "/workspace" {
        return Err("cannot modify the workspace root");
    }
    let name = path.rsplit('/').next().unwrap_or("");
    if name.is_empty() || is_internal_workspace_entry(name) {
        return Err("cannot modify this path");
    }
    Ok(path)
}

pub fn workspace_rename_target(path: &str, new_name: &str) -> Result<String, &'static str> {
    let new_name = new_name.trim();
    if new_name.is_empty() || new_name.len() > 255 {
        return Err("name must be between 1 and 255 characters");
    }
    if new_name.contains('/') || new_name.contains('\0') {
        return Err("name must not contain slashes");
    }
    if is_internal_workspace_entry(new_name) {
        return Err("cannot use a reserved name");
    }
    let path = validate_workspace_mutation_path(path)?;
    let parent = path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .unwrap_or("/workspace");
    Ok(format!("{parent}/{new_name}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::computer::WorkspaceEntry;

    #[test]
    fn hides_elsewhere_markers_not_other_dotfiles() {
        assert!(is_internal_workspace_entry(".elsewhere-bootstrap"));
        assert!(is_internal_workspace_entry(".elsewhere-ready"));
        assert!(!is_internal_workspace_entry(".env"));
        let entries = filter_workspace_listing(vec![
            WorkspaceEntry {
                name: ".elsewhere-bootstrap".into(),
                path: "/workspace/.elsewhere-bootstrap".into(),
                is_dir: false,
            },
            WorkspaceEntry {
                name: "hello_world".into(),
                path: "/workspace/results/run/hello_world".into(),
                is_dir: false,
            },
        ]);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "hello_world");
    }

    #[test]
    fn mutation_paths_reject_escape_and_reserved() {
        assert!(normalize_workspace_path("/workspace/a").is_ok());
        assert!(normalize_workspace_path("/workspace/docs").is_ok());
        assert!(validate_workspace_readable_path("/workspace/docs/readme.md").is_ok());
        assert!(validate_workspace_readable_path("/workspace/.elsewhere-bootstrap").is_err());
        assert!(validate_workspace_mutation_path("/workspace").is_err());
        assert!(validate_workspace_mutation_path("/workspace/.elsewhere-bootstrap").is_err());
        assert!(normalize_workspace_path("/workspace/../etc/passwd").is_err());
        assert!(normalize_workspace_path("/etc/passwd").is_err());
        assert_eq!(
            workspace_rename_target("/workspace/docs/readme.md", "notes.md").as_deref(),
            Ok("/workspace/docs/notes.md")
        );
    }
}
