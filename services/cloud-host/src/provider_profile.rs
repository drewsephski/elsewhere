//! Ownership mapping for durable, host-only Codex profiles.
use std::path::PathBuf;

use crate::{auth::LEGACY_LOCAL_OWNER, config::Config, error::ApiError};

pub async fn profile_for_owner(
    pool: &sqlx::PgPool,
    config: &Config,
    owner_id: &str,
) -> Result<Option<PathBuf>, ApiError> {
    // Preserve the explicitly trusted local development path only.
    if owner_id == LEGACY_LOCAL_OWNER {
        return Ok(None);
    }
    let root = config.codex_profiles_dir.as_ref().ok_or_else(|| {
        ApiError::Conflict("ChatGPT connection is not configured on this deployment".into())
    })?;
    if !root.is_absolute() || root.starts_with("/workspace") {
        return Err(ApiError::Conflict(
            "ChatGPT profiles require ELSEWHERE_CODEX_PROFILES_DIR to be an absolute path on a host-only volume (not inside an agent sandbox); e.g. $HOME/.elsewhere/codex-profiles"
                .into(),
        ));
    }
    let (profile_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO provider_profiles (owner_id, profile_id) VALUES ($1, $2) \
         ON CONFLICT (owner_id) DO UPDATE SET owner_id = EXCLUDED.owner_id RETURNING profile_id",
    )
    .bind(owner_id)
    .bind(uuid::Uuid::new_v4())
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    // Never interpolate an identity or client-supplied path into a filesystem path.
    let path = root.join(profile_id.to_string());
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(&path)
        .map_err(|_| ApiError::Internal("ChatGPT profile storage unavailable".into()))?;
    Ok(Some(path))
}
