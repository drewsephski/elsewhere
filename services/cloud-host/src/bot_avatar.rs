use crate::error::ApiError;

pub const DEFAULT_AVATAR_ID: &str = "sky-wisp";

const ALLOWED: &[&str] = &[
    "sky-wisp",
    "violet-kitty",
    "amber-puff",
    "emerald-bunny",
    "rose-sprout",
    "teal-wisp",
    "coral-puff",
    "indigo-kitty",
    "lime-sprout",
    "slate-bunny",
    "sunset-sprout",
    "berry-puff",
];

pub fn normalize_avatar_id(raw: Option<&str>) -> Result<String, ApiError> {
    let id = raw
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_AVATAR_ID);
    if ALLOWED.contains(&id) {
        Ok(id.to_string())
    } else {
        Err(ApiError::Validation(
            "avatarId is not a supported bot avatar".into(),
        ))
    }
}
