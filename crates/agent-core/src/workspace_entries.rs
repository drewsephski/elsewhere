//! Workspace listing rules shared by guest computers and the cloud API.

/// Reserved Elsewhere housekeeping paths (not user-facing deliverables).
pub fn is_internal_workspace_entry(name: &str) -> bool {
    name.starts_with(".elsewhere-")
}

pub fn filter_workspace_listing(entries: impl IntoIterator<Item = crate::computer::WorkspaceEntry>) -> Vec<crate::computer::WorkspaceEntry> {
    entries
        .into_iter()
        .filter(|entry| !is_internal_workspace_entry(&entry.name))
        .collect()
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
}
