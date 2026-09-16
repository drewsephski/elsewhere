//! Host-only Chromium profile storage keyed by computer (workspace sandbox).

use std::path::{Path, PathBuf};

use crate::{config::Config, error::ApiError};

const PROFILE_DIR_MODE: u32 = 0o700;

fn validate_computer_id(computer_id: &str) -> Result<(), ApiError> {
    if computer_id.is_empty() || computer_id.len() > 128 {
        return Err(ApiError::Validation("invalid computer id".into()));
    }
    if computer_id.contains('/') || computer_id.contains('\\') || computer_id.contains("..") {
        return Err(ApiError::Validation("invalid computer id".into()));
    }
    Ok(())
}

fn browser_profiles_root(config: &Config) -> Result<&Path, ApiError> {
    let root = config.browser_profiles_dir.as_ref().ok_or_else(|| {
        ApiError::Conflict(
            "Browser sign-in persistence is not configured on this deployment".into(),
        )
    })?;
    if !root.is_absolute() || root.starts_with("/workspace") {
        return Err(ApiError::Internal(
            "Browser profiles require an absolute host-only directory".into(),
        ));
    }
    Ok(root.as_path())
}

async fn ensure_computer_owned(
    pool: &sqlx::PgPool,
    owner_id: &str,
    computer_id: &str,
) -> Result<(), ApiError> {
    validate_computer_id(computer_id)?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM sandboxes
            WHERE id = $1 AND owner_id = $2 AND state <> 'archived'
        )",
    )
    .bind(computer_id)
    .bind(owner_id)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    if !exists {
        return Err(ApiError::NotFound);
    }
    Ok(())
}

fn create_profile_dir(root: &Path, profile_id: uuid::Uuid) -> Result<PathBuf, ApiError> {
    let path = root.join(profile_id.to_string());
    if path.is_dir() {
        return Ok(path);
    }
    if path.exists() {
        return Err(ApiError::Internal(
            "Browser profile storage path is not a directory".into(),
        ));
    }
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(PROFILE_DIR_MODE);
    }
    builder
        .create(&path)
        .map_err(|e| ApiError::Internal(format!("Browser profile storage unavailable: {e}")))?;
    Ok(path)
}

/// Resolve or allocate the host directory for this computer's Chromium user data.
pub async fn profile_for_computer(
    pool: &sqlx::PgPool,
    config: &Config,
    owner_id: &str,
    computer_id: &str,
) -> Result<PathBuf, ApiError> {
    ensure_computer_owned(pool, owner_id, computer_id).await?;
    let root = browser_profiles_root(config)?;
    let (profile_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO browser_profiles (computer_id, owner_id, profile_id) VALUES ($1, $2, $3) \
         ON CONFLICT (computer_id) DO UPDATE SET owner_id = EXCLUDED.owner_id \
         RETURNING profile_id",
    )
    .bind(computer_id)
    .bind(owner_id)
    .bind(uuid::Uuid::new_v4()) // used only on first insert; conflicts keep existing profile_id
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    create_profile_dir(root, profile_id)
}

/// Wipe sign-in state: rotate the opaque profile id, delete host bytes, caller clears guest profile.
pub async fn reset_profile_for_computer(
    pool: &sqlx::PgPool,
    config: &Config,
    owner_id: &str,
    computer_id: &str,
) -> Result<(), ApiError> {
    ensure_computer_owned(pool, owner_id, computer_id).await?;
    let root = browser_profiles_root(config)?;

    let old: Option<(uuid::Uuid,)> = sqlx::query_as(
        "SELECT profile_id FROM browser_profiles WHERE computer_id = $1 AND owner_id = $2",
    )
    .bind(computer_id)
    .bind(owner_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let new_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO browser_profiles (computer_id, owner_id, profile_id) VALUES ($1, $2, $3) \
         ON CONFLICT (computer_id) DO UPDATE SET profile_id = EXCLUDED.profile_id, owner_id = EXCLUDED.owner_id",
    )
    .bind(computer_id)
    .bind(owner_id)
    .bind(new_id)
    .execute(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    if let Some((old_id,)) = old {
        let old_path = root.join(old_id.to_string());
        if old_path.exists() {
            std::fs::remove_dir_all(&old_path).map_err(|e| {
                ApiError::Internal(format!("could not remove old browser profile: {e}"))
            })?;
        }
    }

    create_profile_dir(root, new_id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_pathological_computer_ids() {
        assert!(validate_computer_id("").is_err());
        assert!(validate_computer_id("../x").is_err());
        assert!(validate_computer_id("a/b").is_err());
        assert!(validate_computer_id("valid-id").is_ok());
    }

    #[test]
    fn reuses_existing_profile_directory() {
        let root = std::env::temp_dir().join(format!(
            "elsewhere-browser-profile-reuse-{}",
            uuid::Uuid::new_v4()
        ));
        let profile_id = uuid::Uuid::new_v4();
        let first = create_profile_dir(&root, profile_id).expect("first create");
        let second = create_profile_dir(&root, profile_id).expect("reuse existing");
        assert_eq!(first, second);
        std::fs::remove_dir_all(root).ok();
    }
}
