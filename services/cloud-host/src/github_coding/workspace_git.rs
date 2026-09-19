use agent_core::{AgentComputer, ComputerError, GithubCodingError};

use crate::redact::redact_secrets;

const GIT_UNAVAILABLE: &str =
    "This workspace does not support local Git metadata required for GitHub coding.";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GitFileChange {
    pub path: String,
    pub mode: String,
    pub deleted: bool,
    pub bytes: Option<Vec<u8>>,
}

pub async fn reset_checkout_dir(
    computer: &dyn AgentComputer,
    checkout_path: &str,
) -> Result<(), GithubCodingError> {
    let cmd = format!(
        "rm -rf {} && mkdir -p {}",
        shell_quote(checkout_path),
        shell_quote(checkout_path)
    );
    exec_ok(computer, &cmd).await
}

pub async fn init_baseline_repo(
    computer: &dyn AgentComputer,
    checkout_path: &str,
) -> Result<String, GithubCodingError> {
    let script = format!(
        "cd {} && git init -q && git config user.email 'bot@elsewhere.local' && git config user.name 'Elsewhere Bot' && git add -A && git commit -q -m 'elsewhere baseline' && git rev-parse HEAD",
        shell_quote(checkout_path)
    );
    let out = exec_stdout(computer, &script).await?;
    let sha = out.lines().last().unwrap_or("").trim();
    if sha.len() != 40 {
        return Err(GithubCodingError::Provider(
            "failed to create local Git baseline".into(),
        ));
    }
    Ok(sha.to_string())
}

pub async fn working_tree_fingerprint(
    computer: &dyn AgentComputer,
    checkout_path: &str,
    baseline_sha: &str,
) -> Result<String, GithubCodingError> {
    let changes = collect_publish_changes(computer, checkout_path, baseline_sha).await?;
    Ok(super::publish_snapshot::fingerprint_changes(baseline_sha, &changes))
}

pub async fn collect_publish_changes(
    computer: &dyn AgentComputer,
    checkout_path: &str,
    baseline_sha: &str,
) -> Result<Vec<GitFileChange>, GithubCodingError> {
    let diff_script = format!(
        "cd {} && git diff --name-status -z --no-renames {}",
        shell_quote(checkout_path),
        shell_quote(baseline_sha)
    );
    let diff_out = exec_stdout(computer, &diff_script).await?;
    let untracked_script = format!(
        "cd {} && git ls-files --others --exclude-standard -z",
        shell_quote(checkout_path)
    );
    let untracked_out = exec_stdout(computer, &untracked_script).await?;

    let mut paths: Vec<PathMutation> = Vec::new();
    parse_name_status_z(&diff_out, &mut paths);
    for path in parse_nul_paths(&untracked_out) {
        paths.push(PathMutation {
            path,
            status: 'A',
        });
    }
    paths.sort_by(|a, b| a.path.cmp(&b.path));
    paths.dedup_by(|a, b| a.path == b.path);

    let mut changes = Vec::new();
    for entry in paths {
        let path = entry.path;
        if entry.status == 'D' {
            let mode = baseline_blob_mode(computer, checkout_path, baseline_sha, &path).await?;
            changes.push(GitFileChange {
                path,
                mode,
                deleted: true,
                bytes: None,
            });
            continue;
        }
        let full = format!("{}/{}", checkout_path.trim_end_matches('/'), path);
        let mode = worktree_file_mode(computer, &full).await?;
        let bytes = computer
            .read_file(&full)
            .await
            .map_err(map_computer_error)?;
        changes.push(GitFileChange {
            path,
            mode,
            deleted: false,
            bytes: Some(bytes),
        });
    }
    Ok(changes)
}

struct PathMutation {
    path: String,
    status: char,
}

fn parse_nul_paths(raw: &str) -> Vec<String> {
    raw.split('\0')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn parse_name_status_z(raw: &str, out: &mut Vec<PathMutation>) {
    let parts: Vec<&str> = raw.split('\0').filter(|s| !s.is_empty()).collect();
    let mut i = 0;
    while i < parts.len() {
        let token = parts[i];
        if token.len() == 1 {
            if i + 1 >= parts.len() {
                break;
            }
            let status = token.chars().next().unwrap_or('M');
            out.push(PathMutation {
                path: parts[i + 1].to_string(),
                status,
            });
            i += 2;
            continue;
        }
        let status = token.chars().next().unwrap_or('M');
        let path = token
            .strip_prefix(&format!("{status}\t"))
            .or_else(|| token.strip_prefix(&format!("{status} ")))
            .unwrap_or(token)
            .to_string();
        out.push(PathMutation { path, status });
        i += 1;
    }
}

async fn baseline_blob_mode(
    computer: &dyn AgentComputer,
    checkout_path: &str,
    baseline_sha: &str,
    relative: &str,
) -> Result<String, GithubCodingError> {
    let script = format!(
        "cd {} && git ls-tree {} -- {} | awk '{{print $1}}'",
        shell_quote(checkout_path),
        shell_quote(baseline_sha),
        shell_quote(relative)
    );
    let out = exec_stdout(computer, &script).await?;
    let mode = out.lines().next().unwrap_or("100644").trim();
    if mode == "100755" || mode == "100644" {
        return Ok(mode.to_string());
    }
    Ok("100644".into())
}

async fn worktree_file_mode(computer: &dyn AgentComputer, full_path: &str) -> Result<String, GithubCodingError> {
    let script = format!(
        "if [ -x {} ]; then echo 100755; else echo 100644; fi",
        shell_quote(full_path)
    );
    let out = exec_stdout(computer, &script).await?;
    let mode = out.lines().next().unwrap_or("100644").trim();
    if mode == "100755" {
        Ok("100755".into())
    } else {
        Ok("100644".into())
    }
}

pub async fn assert_git_available(computer: &dyn AgentComputer) -> Result<(), GithubCodingError> {
    let result = computer
        .exec("git --version")
        .await
        .map_err(map_computer_error)?;
    if !result.ok {
        return Err(GithubCodingError::Validation(GIT_UNAVAILABLE.into()));
    }
    Ok(())
}

async fn exec_ok(computer: &dyn AgentComputer, command: &str) -> Result<(), GithubCodingError> {
    let result = computer.exec(command).await.map_err(map_computer_error)?;
    if !result.ok {
        return Err(GithubCodingError::Provider(format!(
            "command failed ({}): {}",
            result.exit_code,
            redact_secrets(&result.stderr)
        )));
    }
    Ok(())
}

async fn exec_stdout(
    computer: &dyn AgentComputer,
    command: &str,
) -> Result<String, GithubCodingError> {
    let result = computer.exec(command).await.map_err(map_computer_error)?;
    if !result.ok {
        if result.stderr.contains("not found") || result.stderr.contains("git:") {
            return Err(GithubCodingError::Validation(GIT_UNAVAILABLE.into()));
        }
        return Err(GithubCodingError::Provider(format!(
            "command failed ({}): {}",
            result.exit_code,
            redact_secrets(&result.stderr)
        )));
    }
    Ok(result.stdout)
}

fn map_computer_error(err: ComputerError) -> GithubCodingError {
    GithubCodingError::Provider(err.to_string())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
