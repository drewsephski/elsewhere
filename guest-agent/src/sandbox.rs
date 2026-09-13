//! Path and command restrictions for the guest RPC surface.

use std::ffi::CString;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, FromRawFd};

/// Persistent VM data directory (see `scripts/build-guest-disk.sh`).
pub const WORKSPACE_ROOT: &str = "/workspace";

const MAX_COMMAND_LEN: usize = 64 * 1024;

/// Directory prefixes that must not appear as absolute paths in shell commands.
const EXEC_BLOCKED_PREFIXES: &[&str] = &[
    "/etc/",
    "/proc/",
    "/sys/",
    "/dev/",
    "/run/",
    "/var/run/",
    "/root/",
];

/// Exact absolute paths (or paths under these prefixes) that must not be referenced.
const EXEC_BLOCKED_EXACT: &[&str] = &["/root", "/usr/local/bin/gptbot-guest-agent"];

/// Resolve `path` to an absolute path confined to `WORKSPACE_ROOT` (lexical normalization only).
pub fn resolve_workspace_path(path: &str) -> Result<PathBuf, String> {
    if path.is_empty() {
        return Err("path must not be empty".into());
    }
    if path.contains('\0') {
        return Err("path contains invalid characters".into());
    }

    let workspace = Path::new(WORKSPACE_ROOT);
    let input = Path::new(path);
    let joined = if input.is_absolute() {
        input.to_path_buf()
    } else {
        workspace.join(input)
    };

    let normalized = normalize_path(&joined)?;
    if !path_starts_with_workspace(&normalized) {
        return Err("path is outside the workspace sandbox".into());
    }
    Ok(normalized)
}

/// Read a file only if its canonical location stays under the workspace root.
pub fn read_workspace_file(path: &str) -> Result<String, String> {
    let resolved = resolve_workspace_path(path)?;
    let workspace_canon = workspace_canonical_root()?;
    let canonical = fs::canonicalize(&resolved).map_err(|e| e.to_string())?;
    if !canonical.starts_with(&workspace_canon) {
        return Err("path resolves outside the workspace sandbox".into());
    }
    fs::read_to_string(&canonical).map_err(|e| e.to_string())
}

/// Write a file only under the workspace; creates parent directories as needed.
pub fn write_workspace_file(path: &str, content: &str) -> Result<(), String> {
    let resolved = resolve_workspace_path(path)?;
    let workspace_canon = workspace_canonical_root()?;

    let file_name = resolved
        .file_name()
        .ok_or_else(|| "invalid path".to_string())?;
    let parent = resolved
        .parent()
        .ok_or_else(|| "invalid path".to_string())?;

    #[cfg(unix)]
    {
        let create_missing = !parent.exists();
        let parent_dir =
            open_workspace_parent(parent, &workspace_canon, create_missing)?;
        write_file_at_parent(&parent_dir, file_name, content)?;
        return Ok(());
    }

    #[cfg(not(unix))]
    {
        let parent_canon = canonical_parent_under_workspace(parent, &workspace_canon)?;
        write_workspace_file_non_unix(&parent_canon, file_name, content, &workspace_canon)?;
        return Ok(());
    }
}

/// Non-Unix fallback: parent directory is already canonical and under the workspace.
#[cfg(not(unix))]
fn write_workspace_file_non_unix(
    parent_canon: &Path,
    file_name: &std::ffi::OsStr,
    content: &str,
    workspace_canon: &Path,
) -> Result<(), String> {
    let write_path = parent_canon.join(file_name);

    if write_path.exists() || fs::symlink_metadata(&write_path).is_ok() {
        let canonical = canonical_path_under_workspace(&write_path, workspace_canon)?;
        fs::write(&canonical, content).map_err(|e| e.to_string())?;
        return Ok(());
    }

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&write_path)
        .map_err(|e| e.to_string())?;
    file.write_all(content.as_bytes()).map_err(|e| e.to_string())?;

    match canonical_path_under_workspace(&write_path, workspace_canon) {
        Ok(_) => Ok(()),
        Err(err) => {
            let _ = fs::remove_file(&write_path);
            Err(err)
        }
    }
}

#[cfg(not(unix))]
fn canonical_path_under_workspace(
    path: &Path,
    workspace_canon: &Path,
) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(path).map_err(|e| e.to_string())?;
    if !canonical.starts_with(workspace_canon) {
        return Err("path resolves outside the workspace sandbox".into());
    }
    Ok(canonical)
}

#[cfg(not(unix))]
fn canonical_parent_under_workspace(
    parent: &Path,
    workspace_canon: &Path,
) -> Result<PathBuf, String> {
    if parent.exists() {
        let parent_canon = fs::canonicalize(parent).map_err(|e| e.to_string())?;
        if !parent_canon.starts_with(workspace_canon) {
            return Err("parent path resolves outside the workspace sandbox".into());
        }
        return Ok(parent_canon);
    }

    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let parent_canon = fs::canonicalize(parent).map_err(|e| e.to_string())?;
    if !parent_canon.starts_with(workspace_canon) {
        return Err("parent path resolves outside the workspace sandbox".into());
    }
    Ok(parent_canon)
}

/// Walk from the canonical workspace root to `parent` using `openat`/`mkdirat` without following symlinks.
#[cfg(unix)]
fn open_workspace_parent(
    parent: &Path,
    workspace_canon: &Path,
    create_missing: bool,
) -> Result<fs::File, String> {
    let relative = path_relative_to_workspace_root(parent)?;

    let mut dir = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(workspace_canon)
        .map_err(|e| e.to_string())?;

    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err("invalid path component".into());
        };
        match open_directory_at(&dir, name) {
            Ok(sub) => dir = sub,
            Err(err) if create_missing && err.kind() == std::io::ErrorKind::NotFound => {
                mkdir_at(&dir, name)?;
                dir = open_directory_at(&dir, name).map_err(|e| e.to_string())?;
            }
            Err(err) => return Err(err.to_string()),
        }
    }

    let parent_canon = fd_canonical_path(&dir)?;
    if !parent_canon.starts_with(workspace_canon) {
        return Err("parent path resolves outside the workspace sandbox".into());
    }
    Ok(dir)
}

#[cfg(unix)]
fn path_relative_to_workspace_root(path: &Path) -> Result<PathBuf, String> {
    if path == Path::new(WORKSPACE_ROOT) {
        return Ok(PathBuf::new());
    }
    let relative = path
        .strip_prefix(WORKSPACE_ROOT)
        .map_err(|_| "parent path resolves outside the workspace sandbox".to_string())?;
    Ok(relative
        .strip_prefix("/")
        .unwrap_or(relative)
        .to_path_buf())
}

#[cfg(unix)]
fn osstr_to_cstring(name: &std::ffi::OsStr) -> Result<CString, String> {
    let bytes = name.as_bytes();
    if bytes.contains(&0) {
        return Err("path contains invalid characters".into());
    }
    CString::new(bytes).map_err(|_| "path contains invalid characters".into())
}

#[cfg(unix)]
fn open_directory_at(base: &fs::File, name: &std::ffi::OsStr) -> Result<fs::File, std::io::Error> {
    let cname = osstr_to_cstring(name).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let fd = unsafe {
        libc::openat(
            base.as_raw_fd(),
            cname.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { fs::File::from_raw_fd(fd) })
}

#[cfg(unix)]
fn mkdir_at(base: &fs::File, name: &std::ffi::OsStr) -> Result<(), String> {
    let cname = osstr_to_cstring(name)?;
    let rc = unsafe { libc::mkdirat(base.as_raw_fd(), cname.as_ptr(), 0o755) };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() == std::io::ErrorKind::AlreadyExists {
            return Ok(());
        }
        return Err(err.to_string());
    }
    Ok(())
}

#[cfg(unix)]
fn fd_canonical_path(dir: &fs::File) -> Result<PathBuf, String> {
    let proc = format!("/proc/self/fd/{}", dir.as_raw_fd());
    fs::canonicalize(proc).map_err(|e| e.to_string())
}

/// Open the target file relative to a verified parent directory (no symlink following).
#[cfg(unix)]
fn write_file_at_parent(parent: &fs::File, file_name: &std::ffi::OsStr, content: &str) -> Result<(), String> {
    let cname = osstr_to_cstring(file_name)?;
    let truncate_flags =
        libc::O_WRONLY | libc::O_TRUNC | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    let fd = unsafe { libc::openat(parent.as_raw_fd(), cname.as_ptr(), truncate_flags, 0) };
    if fd < 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() != std::io::ErrorKind::NotFound {
            return Err(err.to_string());
        }
        let create_flags = libc::O_WRONLY
            | libc::O_CREAT
            | libc::O_EXCL
            | libc::O_NOFOLLOW
            | libc::O_CLOEXEC;
        let fd = unsafe {
            libc::openat(parent.as_raw_fd(), cname.as_ptr(), create_flags, 0o644)
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        return write_fd(fd, content);
    }
    write_fd(fd, content)
}

#[cfg(unix)]
fn write_fd(fd: i32, content: &str) -> Result<(), String> {
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd, &mut stat) } != 0 {
        let err = std::io::Error::last_os_error().to_string();
        unsafe {
            libc::close(fd);
        }
        return Err(err);
    }
    if stat.st_mode & libc::S_IFMT != libc::S_IFREG {
        unsafe {
            libc::close(fd);
        }
        return Err("path is not a regular file".into());
    }
    let mut file = unsafe { fs::File::from_raw_fd(fd) };
    file.write_all(content.as_bytes()).map_err(|e| e.to_string())
}

pub fn validate_exec_command(command: &str) -> Result<(), String> {
    if command.is_empty() {
        return Err("missing params.command".into());
    }
    if command.len() > MAX_COMMAND_LEN {
        return Err("command exceeds maximum length".into());
    }
    if command.contains('\0') {
        return Err("command contains invalid characters".into());
    }

    for candidate in extract_absolute_path_literals(command) {
        let normalized = normalize_path(Path::new(&candidate))?;
        if let Some(blocked) = blocked_path_match(&normalized) {
            return Err(format!(
                "command references a blocked location ({blocked})"
            ));
        }
    }
    Ok(())
}

/// Scan the command for absolute path literals (not whole-string substring checks).
fn extract_absolute_path_literals(command: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let bytes = command.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'/' {
            i += 1;
            continue;
        }
        if i > 0 && is_path_char(bytes[i - 1]) {
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        while i < bytes.len() && is_path_char(bytes[i]) {
            i += 1;
        }
        let mut end = i;
        while end > start && is_path_trailing_punctuation(bytes[end - 1]) {
            end -= 1;
        }
        if end > start {
            if let Ok(segment) = std::str::from_utf8(&bytes[start..end]) {
                paths.push(segment.to_string());
            }
        }
    }
    paths
}

fn is_path_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'/' || byte == b'.' || byte == b'_' || byte == b'-'
}

fn is_path_trailing_punctuation(byte: u8) -> bool {
    matches!(byte, b'"' | b'\'' | b')' | b']' | b'}' | b',' | b';' | b':')
}

fn blocked_path_match(path: &Path) -> Option<&'static str> {
    let path_str = path.to_string_lossy();
    let lower = path_str.to_ascii_lowercase();

    for prefix in EXEC_BLOCKED_PREFIXES {
        let prefix_lower = prefix.to_ascii_lowercase();
        if lower == prefix_lower.trim_end_matches('/')
            || lower.starts_with(&prefix_lower)
        {
            return Some(prefix);
        }
    }

    for exact in EXEC_BLOCKED_EXACT {
        let exact_lower = exact.to_ascii_lowercase();
        if lower == exact_lower || lower.starts_with(&format!("{exact_lower}/")) {
            return Some(exact);
        }
    }

    if path_contains_dot_ssh(&lower) {
        return Some("/.ssh");
    }

    None
}

fn path_contains_dot_ssh(path: &str) -> bool {
    path == "/.ssh"
        || path.starts_with("/.ssh/")
        || path.contains("/.ssh/")
        || path.ends_with("/.ssh")
}

pub fn workspace_dir_for_exec() -> PathBuf {
    PathBuf::from(WORKSPACE_ROOT)
}

fn workspace_canonical_root() -> Result<PathBuf, String> {
    let root = Path::new(WORKSPACE_ROOT);
    if !root.exists() {
        fs::create_dir_all(root).map_err(|e| e.to_string())?;
    }
    fs::canonicalize(root).map_err(|e| e.to_string())
}

fn normalize_path(path: &Path) -> Result<PathBuf, String> {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => {
                if out.as_os_str().is_empty() {
                    out.push("/");
                }
            }
            Component::CurDir => {}
            Component::Normal(part) => out.push(part),
            Component::ParentDir => {
                if !out.pop() {
                    return Err("path escapes above filesystem root".into());
                }
            }
        }
    }
    Ok(out)
}

fn path_starts_with_workspace(path: &Path) -> bool {
    let workspace = Path::new(WORKSPACE_ROOT);
    path == workspace || path.starts_with(workspace)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Mutex, OnceLock};

    fn with_temp_workspace<F: FnOnce()>(f: F) {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let dir = std::env::temp_dir().join(format!(
            "gptbot-guest-sandbox-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        // Tests use a subdirectory layout mirroring /workspace via chroot-less prefix checks.
        // We only test normalize + blocklist here on macOS; linux integration uses /workspace.
        f();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_traversal_outside_workspace() {
        assert!(resolve_workspace_path("/etc/passwd").is_err());
        assert!(resolve_workspace_path("/workspace/../etc/passwd").is_err());
        assert!(resolve_workspace_path("../../etc/passwd").is_err());
    }

    #[test]
    fn allows_workspace_relative_paths() {
        let p = resolve_workspace_path("proof.txt").expect("ok");
        assert_eq!(p, PathBuf::from("/workspace/proof.txt"));
        let p = resolve_workspace_path("/workspace/nested/a.txt").expect("ok");
        assert_eq!(p, PathBuf::from("/workspace/nested/a.txt"));
    }

    #[test]
    fn exec_blocklist_catches_sensitive_paths() {
        assert!(validate_exec_command("cat /etc/passwd").is_err());
        assert!(validate_exec_command("cat /root/.profile").is_err());
        assert!(validate_exec_command("echo hi").is_ok());
    }

    #[test]
    fn exec_blocklist_does_not_false_positive_on_similar_paths() {
        assert!(validate_exec_command("ls /rooted_backup").is_ok());
        assert!(validate_exec_command("ls /root_data/foo").is_ok());
        assert!(validate_exec_command("/usr/local/roottools/scan").is_ok());
    }

    #[test]
    fn rejects_oversized_command() {
        let huge = "a".repeat(MAX_COMMAND_LEN + 1);
        assert!(validate_exec_command(&huge).is_err());
    }

    #[test]
    fn workspace_path_empty_rejected() {
        with_temp_workspace(|| {
            assert!(resolve_workspace_path("").is_err());
        });
    }
}
