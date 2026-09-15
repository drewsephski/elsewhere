use thiserror::Error;

#[derive(Debug, Error)]
pub enum SkillPackageError {
    #[error("validation: {0}")]
    Validation(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

impl SkillPackageError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }
}
