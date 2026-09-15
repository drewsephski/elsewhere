mod error;
mod limits;
mod materialize;
mod package;
mod parse;
mod responses;

pub use error::SkillPackageError;
pub use limits::*;
pub use materialize::{materialize_agents_skills, native_skills_relative_dir, NativeSkillsLayout};
pub use package::{SkillPackage, SkillPackageFile};
pub use parse::ValidatedSkillFrontmatter;
pub use responses::append_skills_to_instructions;

#[cfg(test)]
mod tests;
