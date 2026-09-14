use std::fmt;

/// Stable Elsewhere user identifier (Better Auth JWT `sub` or legacy internal principal).
pub const LEGACY_LOCAL_OWNER: &str = "legacy-local";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthKind {
    Jwt,
    InternalToken,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    pub subject: String,
    pub auth_kind: AuthKind,
}

impl Principal {
    pub fn legacy_local() -> Self {
        Self {
            subject: LEGACY_LOCAL_OWNER.to_string(),
            auth_kind: AuthKind::InternalToken,
        }
    }

    pub fn owner_id(&self) -> &str {
        &self.subject
    }
}

impl fmt::Display for Principal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.subject)
    }
}
