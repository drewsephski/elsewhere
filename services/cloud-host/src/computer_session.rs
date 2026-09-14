use std::sync::Arc;
use std::time::Duration;

use agent_core::AgentComputer;
use sprite_computer::{default_deny_network_policy, SpriteComputer, SpriteComputerConfig};

use crate::config::Config;
use crate::error::ApiError;
use crate::runner::sprite_resource_for_computer;

pub async fn connect_sprite_computer(
    config: &Config,
    pool: &sqlx::PgPool,
    owner_id: &str,
    computer_id: &str,
) -> Result<Arc<dyn AgentComputer>, ApiError> {
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

    if !config.browser_enabled {
        return Err(ApiError::Validation(
            "browser preview is disabled on this host".into(),
        ));
    }

    let sprite_name = sprite_resource_for_computer(pool, computer_id)
        .await
        .map_err(|e| ApiError::Internal(e))?;

    let computer = SpriteComputer::new(SpriteComputerConfig {
        base_url: config.sprites_api_base.clone(),
        token: config.sprite_token.clone(),
        sprite_name,
        workspace_root: "/workspace".into(),
        request_timeout: Duration::from_secs(120),
        auto_create: true,
        network_policy: default_deny_network_policy(),
        exec_timeout: Duration::from_secs(60),
        browser_enabled: true,
        browser_exec_timeout: Duration::from_secs(45),
    })
    .map_err(|e| ApiError::Internal(format!("SpriteComputer: {e}")))?;

    Ok(Arc::new(computer))
}
