use agent_core::{AgentComputer, ComputerError, GithubCodingError};
use sha2::{Digest, Sha256};

use crate::redact::redact_secrets;

const GIT_UNAVAILABLE: &str =
    "This workspace does not support local Git metadata required for GitHub coding.";

#[derive(Debug, Clone, PartialEq, Eq)]
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
) -> Result<String, GithubCodingError> {
    let head_script = format!(
        "cd {} && git rev-parse HEAD",
        shell_quote(checkout_path)
    );
    let head = exec_stdout(computer, &head_script).await?;
    let changes = collect_publish_changes(computer, checkout_path).await?;
    let mut hasher = Sha256::new();
    hasher.update(head.trim().as_bytes());
    for change in changes {
        hasher.update(change.path.as_bytes());
        hasher.update(change.mode.as_bytes());
        hasher.update([u8::from(change.deleted)]);
        if let Some(bytes) = &change.bytes {
            hasher.update(bytes);
        }
    }
    Ok(hex::encode(hasher.finalize()))
}

pub async fn collect_publish_changes(
    computer: &dyn AgentComputer,
    checkout_path: &str,
) -> Result<Vec<GitFileChange>, GithubCodingError> {
    let script = format!(
        "cd {} && git status --porcelain=v1 -z",
        shell_quote(checkout_path)
    );
    let status = exec_stdout(computer, &script).await?;
    let entries = parse_porcelain_z(&status);
    if entries.is_empty() {
        return Ok(Vec::new());
    }
    let mut changes = Vec::new();
    for entry in entries {
        let path = entry.path;
        let full = format!("{}/{}", checkout_path.trim_end_matches('/'), path);
        if entry.deleted {
            changes.push(GitFileChange {
                path,
                mode: "100644".into(),
                deleted: true,
                bytes: None,
            });
            continue;
        }
        let mode = file_mode(computer, checkout_path, &path).await?;
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

struct PorcelainEntry {
    path: String,
    deleted: bool,
}

fn parse_porcelain_z(status: &str) -> Vec<PorcelainEntry> {
    let mut out = Vec::new();
    let mut parts: Vec<&str> = status.split('\0').filter(|s| !s.is_empty()).collect();
    let mut i = 0;
    while i < parts.len() {
        let line = parts[i];
        if line.len() < 4 {
            i += 1;
            continue;
        }
        let xy = &line[..2];
        let path = line[3..].to_string();
        let deleted = xy.contains('D');
        out.push(PorcelainEntry { path, deleted });
        i += 1;
        if xy.starts_with('R') || xy.starts_with('C') {
            i += 1;
        }
    }
    out
}

async fn file_mode(
    computer: &dyn AgentComputer,
    checkout_path: &str,
    relative: &str,
) -> Result<String, GithubCodingError> {
    let script = format!(
        "cd {} && git ls-files -s -- {} | awk '{{print $1}}'",
        shell_quote(checkout_path),
        shell_quote(relative)
    );
    let out = exec_stdout(computer, &script).await?;
    let mode = out.lines().next().unwrap_or("100644").trim();
    if mode == "100755" || mode == "100644" {
        return Ok(mode.to_string());
    }
    Ok("100644".into())
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
