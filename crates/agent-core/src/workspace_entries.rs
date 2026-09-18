//! Workspace listing rules shared by guest computers and the cloud API.

/// Reserved Elsewhere housekeeping paths (not user-facing deliverables).
pub fn is_internal_workspace_entry(name: &str) -> bool {
    name == ".elsewhere" || name.starts_with(".elsewhere-")
}

/// OS/metadata junk that should never appear in the user-facing Files UI.
const HIDDEN_LISTING_NAMES: &[&str] = &[
    ".elsewhere",
    ".DS_Store",
    ".Trashes",
    ".Spotlight-V100",
    ".fseventsd",
    "Thumbs.db",
];

/// Generated or VCS directories that are noise in this product surface.
const HIDDEN_LISTING_DIR_NAMES: &[&str] = &[
    ".git",
    "__MACOSX",
    "node_modules",
    ".next",
    "dist",
    "build",
    "target",
    "coverage",
];

fn eq_ignore_ascii_case(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

/// Whether this directory/file name is hidden from user-facing listings.
pub fn is_hidden_workspace_listing_name(name: &str, is_dir: bool) -> bool {
    if is_internal_workspace_entry(name) {
        return true;
    }
    if HIDDEN_LISTING_NAMES
        .iter()
        .any(|hidden| eq_ignore_ascii_case(name, hidden))
    {
        return true;
    }
    is_dir
        && HIDDEN_LISTING_DIR_NAMES
            .iter()
            .any(|hidden| eq_ignore_ascii_case(name, hidden))
}

fn workspace_path_segments(path: &str) -> impl Iterator<Item = &str> {
    path.split('/')
        .filter(|segment| !segment.is_empty() && *segment != "workspace")
}

fn path_has_hidden_directory_ancestor(path: &str) -> bool {
    let segments: Vec<&str> = workspace_path_segments(path).collect();
    if segments.len() < 2 {
        return false;
    }
    segments[..segments.len() - 1]
        .iter()
        .any(|segment| is_hidden_workspace_listing_name(segment, true))
}

/// User-facing Files visibility. Presentation/filtering only — files are not deleted.
pub fn is_user_visible_workspace_entry(path: &str, name: &str, is_dir: bool) -> bool {
    if is_hidden_workspace_listing_name(name, is_dir) {
        return false;
    }
    !path_has_hidden_directory_ancestor(path)
}

pub fn filter_workspace_listing(
    entries: impl IntoIterator<Item = crate::computer::WorkspaceEntry>,
) -> Vec<crate::computer::WorkspaceEntry> {
    entries
        .into_iter()
        .filter(|entry| is_user_visible_workspace_entry(&entry.path, &entry.name, entry.is_dir))
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

fn leaf_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or("")
}

/// User-visible directory listing path (root allowed; hidden dirs and descendants rejected).
pub fn validate_workspace_list_path(path: &str) -> Result<String, &'static str> {
    let path = normalize_workspace_path(path)?;
    if path == "/workspace" {
        return Ok(path);
    }
    let name = leaf_name(&path);
    if name.is_empty() || !is_user_visible_workspace_entry(&path, name, true) {
        return Err("cannot list this path");
    }
    Ok(path)
}

/// User-visible read path (listing root allowed; reserved markers and hidden descendants rejected).
pub fn validate_workspace_readable_path(path: &str) -> Result<String, &'static str> {
    let path = normalize_workspace_path(path)?;
    if path == "/workspace" {
        return Ok(path);
    }
    let name = leaf_name(&path);
    if name.is_empty() {
        return Err("cannot read this path");
    }
    if is_internal_workspace_entry(name)
        || HIDDEN_LISTING_NAMES
            .iter()
            .any(|hidden| eq_ignore_ascii_case(name, hidden))
        || path_has_hidden_directory_ancestor(&path)
    {
        return Err("cannot read this path");
    }
    Ok(path)
}

/// User-visible file or folder (not `/workspace` root or Elsewhere markers).
pub fn validate_workspace_mutation_path(path: &str) -> Result<String, &'static str> {
    let path = normalize_workspace_path(path)?;
    if path == "/workspace" {
        return Err("cannot modify the workspace root");
    }
    let name = leaf_name(&path);
    if name.is_empty() {
        return Err("cannot modify this path");
    }
    if is_internal_workspace_entry(name)
        || HIDDEN_LISTING_NAMES
            .iter()
            .any(|hidden| eq_ignore_ascii_case(name, hidden))
        || path_has_hidden_directory_ancestor(&path)
    {
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
    if is_internal_workspace_entry(new_name)
        || HIDDEN_LISTING_NAMES
            .iter()
            .any(|hidden| eq_ignore_ascii_case(new_name, hidden))
    {
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

    fn entry(name: &str, path: &str, is_dir: bool) -> WorkspaceEntry {
        WorkspaceEntry {
            name: name.into(),
            path: path.into(),
            is_dir,
        }
    }

    #[test]
    fn hides_elsewhere_markers_not_other_dotfiles() {
        assert!(is_internal_workspace_entry(".elsewhere"));
        assert!(is_internal_workspace_entry(".elsewhere-bootstrap"));
        assert!(is_internal_workspace_entry(".elsewhere-ready"));
        assert!(!is_internal_workspace_entry(".env"));
        assert!(!is_internal_workspace_entry(".gitignore"));
        let entries = filter_workspace_listing(vec![
            entry(
                ".elsewhere-bootstrap",
                "/workspace/.elsewhere-bootstrap",
                false,
            ),
            entry("hello_world", "/workspace/results/run/hello_world", false),
        ]);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "hello_world");
    }

    #[test]
    fn hides_elsewhere_directory_and_descendants() {
        assert!(!is_user_visible_workspace_entry(
            "/workspace/.elsewhere",
            ".elsewhere",
            true,
        ));
        assert!(!is_user_visible_workspace_entry(
            "/workspace/.elsewhere/config.json",
            "config.json",
            false,
        ));
        let entries = filter_workspace_listing(vec![
            entry(".elsewhere", "/workspace/.elsewhere", true),
            entry("config.json", "/workspace/.elsewhere/config.json", false),
            entry("README.md", "/workspace/README.md", false),
        ]);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "README.md");
    }

    #[test]
    fn keeps_useful_dotfiles_and_project_files() {
        let entries = filter_workspace_listing(vec![
            entry(".env.example", "/workspace/.env.example", false),
            entry(".github", "/workspace/.github", true),
            entry(".gitignore", "/workspace/.gitignore", false),
            entry(".cursor", "/workspace/.cursor", true),
            entry("README.md", "/workspace/README.md", false),
            entry("package.json", "/workspace/package.json", false),
            entry("Cargo.toml", "/workspace/Cargo.toml", false),
            entry(".env", "/workspace/.env", false),
        ]);
        assert_eq!(entries.len(), 8);
    }

    #[test]
    fn hides_os_junk_and_generated_noise_directories() {
        let entries = filter_workspace_listing(vec![
            entry(".git", "/workspace/.git", true),
            entry(".DS_Store", "/workspace/.DS_Store", false),
            entry("node_modules", "/workspace/node_modules", true),
            entry("index.js", "/workspace/node_modules/index.js", false),
            entry(".next", "/workspace/.next", true),
            entry("dist", "/workspace/dist", true),
            entry("build", "/workspace/build", true),
            entry("target", "/workspace/target", true),
            entry("coverage", "/workspace/coverage", true),
            entry("__MACOSX", "/workspace/__MACOSX", true),
            entry("Thumbs.db", "/workspace/Thumbs.db", false),
            entry("src", "/workspace/src", true),
        ]);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "src");
    }

    #[test]
    fn generated_noise_rules_do_not_hide_same_named_files() {
        assert!(is_user_visible_workspace_entry(
            "/workspace/dist.ts",
            "dist.ts",
            false,
        ));
        assert!(is_user_visible_workspace_entry(
            "/workspace/build",
            "build",
            false
        ));
        assert!(!is_user_visible_workspace_entry(
            "/workspace/build",
            "build",
            true
        ));
    }

    #[test]
    fn mutation_paths_reject_escape_and_reserved() {
        assert!(normalize_workspace_path("/workspace/a").is_ok());
        assert!(normalize_workspace_path("/workspace/docs").is_ok());
        assert!(validate_workspace_readable_path("/workspace/docs/readme.md").is_ok());
        assert!(validate_workspace_readable_path("/workspace/.elsewhere-bootstrap").is_err());
        assert!(validate_workspace_readable_path("/workspace/.elsewhere/config.json").is_err());
        assert!(validate_workspace_list_path("/workspace/.elsewhere").is_err());
        assert!(validate_workspace_list_path("/workspace/node_modules").is_err());
        assert!(validate_workspace_mutation_path("/workspace").is_err());
        assert!(validate_workspace_mutation_path("/workspace/.elsewhere-bootstrap").is_err());
        assert!(normalize_workspace_path("/workspace/../etc/passwd").is_err());
        assert!(normalize_workspace_path("/etc/passwd").is_err());
        assert_eq!(
            workspace_rename_target("/workspace/docs/readme.md", "notes.md").as_deref(),
            Ok("/workspace/docs/notes.md")
        );
        assert!(workspace_rename_target("/workspace/docs/readme.md", ".elsewhere").is_err());
    }
}
