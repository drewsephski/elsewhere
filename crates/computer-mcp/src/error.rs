use agent_core::ComputerError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ComputerMcpError {
    #[error("computer not provisioned")]
    NotProvisioned,
    #[error("computer unavailable: {0}")]
    Unavailable(String),
    #[error("sandbox rejected: {0}")]
    SandboxRejected(String),
    #[error("malformed arguments: {0}")]
    MalformedArguments(String),
    #[error("execution failed: {0}")]
    ExecutionFailed(String),
    #[error("cancelled")]
    Cancelled,
    #[error("unauthorized")]
    Unauthorized,
    #[error("internal error: {0}")]
    Internal(String),
}

impl ComputerMcpError {
    pub fn from_computer(err: ComputerError) -> Self {
        match err {
            ComputerError::NotProvisioned => Self::NotProvisioned,
            ComputerError::BootFailed(d) | ComputerError::GuestUnavailable(d) => {
                Self::Unavailable(d)
            }
            ComputerError::MalformedArguments(d) => Self::MalformedArguments(d),
            ComputerError::SandboxRejected(d) => Self::SandboxRejected(d),
            ComputerError::ExecutionFailed(d) => Self::ExecutionFailed(d),
            ComputerError::AmbiguousOutcome(d) => Self::ExecutionFailed(d),
            ComputerError::Cancelled => Self::Cancelled,
        }
    }
}
