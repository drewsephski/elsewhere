use thiserror::Error;

#[derive(Debug, Error)]
pub enum CodexProviderError {
    #[error("codex executable not found on PATH")]
    CodexNotInstalled,
    #[error("unsupported Codex version: {0}")]
    UnsupportedCodexVersion(String),
    #[error("unsupported_codex_tool_exposure: {0}")]
    UnsupportedCodexToolExposure(String),
    #[error("app-server process error: {0}")]
    Process(String),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("request timed out: {0}")]
    Timeout(String),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("login error: {0}")]
    Login(String),
    #[error("account error: {0}")]
    Account(String),
    #[error("configuration error: {0}")]
    Config(String),
    #[error("run engine error: {0}")]
    RunEngine(String),
}
