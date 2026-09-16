use std::fmt;

pub const MAX_MEMORY_CONTENT_BYTES: usize = 2048;
pub const MAX_ACTIVE_MEMORIES_PER_BOT: i64 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryKind {
    Preference,
    Fact,
    Project,
    Constraint,
    Workflow,
    Relationship,
    Other,
}

impl MemoryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Preference => "preference",
            Self::Fact => "fact",
            Self::Project => "project",
            Self::Constraint => "constraint",
            Self::Workflow => "workflow",
            Self::Relationship => "relationship",
            Self::Other => "other",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim() {
            "preference" => Ok(Self::Preference),
            "fact" => Ok(Self::Fact),
            "project" => Ok(Self::Project),
            "constraint" => Ok(Self::Constraint),
            "workflow" => Ok(Self::Workflow),
            "relationship" => Ok(Self::Relationship),
            "other" => Ok(Self::Other),
            _ => Err("kind must be preference, fact, project, constraint, workflow, relationship, or other".into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemorySourceKind {
    Manual,
    ExplicitTool,
    Automatic,
}

impl MemorySourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::ExplicitTool => "explicit_tool",
            Self::Automatic => "automatic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryStatus {
    Active,
    Superseded,
    Archived,
}

impl MemoryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Superseded => "superseded",
            Self::Archived => "archived",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim() {
            "active" => Ok(Self::Active),
            "superseded" => Ok(Self::Superseded),
            "archived" => Ok(Self::Archived),
            _ => Err("status must be active, superseded, or archived".into()),
        }
    }
}

impl fmt::Display for MemoryKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

pub fn normalize_content(content: &str) -> String {
    content
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub fn clamp_importance(value: i32) -> i16 {
    value.clamp(1, 5) as i16
}

pub fn clamp_confidence(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}
