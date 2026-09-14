use std::path::PathBuf;
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexAuthMethod {
    ChatGpt,
    ApiKey,
    NotLoggedIn,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexInstallProbe {
    pub executable: PathBuf,
    pub version: Option<String>,
    pub auth: CodexAuthMethod,
}

#[derive(Debug, Error)]
pub enum CodexProbeError {
    #[error("codex executable not found on PATH")]
    NotInstalled,
    #[error("failed to run codex: {0}")]
    Spawn(String),
}

/// Locate `codex` on PATH and read version + login status (CLI interim).
pub fn probe_codex() -> Result<CodexInstallProbe, CodexProbeError> {
    let executable = which_codex()?;
    let version = run_capture(&executable, &["--version"]).ok();
    let status_stdout = run_capture(&executable, &["login", "status"]).unwrap_or_default();
    let auth = parse_login_status(&status_stdout);

    Ok(CodexInstallProbe {
        executable,
        version,
        auth,
    })
}

pub fn parse_login_status(stdout: &str) -> CodexAuthMethod {
    let line = stdout
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");

    let normalized = line.to_ascii_lowercase();
    if normalized.contains("logged in using chatgpt") || normalized.contains("chatgpt") {
        return CodexAuthMethod::ChatGpt;
    }
    if normalized.contains("logged in using api") || normalized.contains("api key") {
        return CodexAuthMethod::ApiKey;
    }
    if normalized.contains("not logged in") || normalized.is_empty() {
        return CodexAuthMethod::NotLoggedIn;
    }
    CodexAuthMethod::Unknown(line.to_string())
}

pub fn prefers_chatgpt_subscription(probe: &CodexInstallProbe) -> bool {
    probe.auth == CodexAuthMethod::ChatGpt
}

fn which_codex() -> Result<PathBuf, CodexProbeError> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join("codex");
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(CodexProbeError::NotInstalled)
}

fn run_capture(executable: &PathBuf, args: &[&str]) -> Result<String, CodexProbeError> {
    let output = Command::new(executable)
        .args(args)
        .output()
        .map_err(|e| CodexProbeError::Spawn(e.to_string()))?;
    if !output.status.success() {
        return Err(CodexProbeError::Spawn(format!(
            "codex {} exited with {}",
            args.join(" "),
            output.status
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_chatgpt_status() {
        assert_eq!(
            parse_login_status("Logged in using ChatGPT\n"),
            CodexAuthMethod::ChatGpt
        );
    }

    #[test]
    fn parse_api_key_status() {
        assert_eq!(
            parse_login_status("Logged in using API key\n"),
            CodexAuthMethod::ApiKey
        );
    }

    #[test]
    fn parse_not_logged_in() {
        assert_eq!(parse_login_status(""), CodexAuthMethod::NotLoggedIn);
    }
}
