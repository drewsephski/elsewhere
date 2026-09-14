//! Shared `SpriteComputer` instances per owner/computer so execution gates cannot be bypassed.

use std::sync::Arc;
use std::time::Duration;

use agent_core::AgentComputer;
use dashmap::DashMap;
use sprite_computer::{default_deny_network_policy, SpriteComputer, SpriteComputerConfig};

use crate::config::Config;
use crate::error::ApiError;
use crate::runner::sprite_resource_for_computer;

#[derive(Clone, Default)]
pub struct ComputerRegistry {
    sprites: Arc<DashMap<(String, String), Arc<SpriteComputer>>>,
}

impl ComputerRegistry {
    pub async fn connect_sprite(
        &self,
        config: &Config,
        pool: &sqlx::PgPool,
        owner_id: &str,
        computer_id: &str,
        browser_enabled: bool,
    ) -> Result<Arc<SpriteComputer>, ApiError> {
        let key = (owner_id.to_string(), computer_id.to_string());
        if let Some(existing) = self.sprites.get(&key) {
            return Ok(existing.clone());
        }

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

        let sprite_name = sprite_resource_for_computer(pool, computer_id)
            .await
            .map_err(ApiError::Internal)?;

        let computer = SpriteComputer::new(SpriteComputerConfig {
            base_url: config.sprites_api_base.clone(),
            token: config.sprite_token.clone(),
            sprite_name,
            workspace_root: "/workspace".into(),
            request_timeout: Duration::from_secs(120),
            auto_create: true,
            network_policy: default_deny_network_policy(),
            exec_timeout: Duration::from_secs(60),
            browser_enabled,
            browser_exec_timeout: Duration::from_secs(45),
        })
        .map_err(|e| ApiError::Internal(format!("SpriteComputer: {e}")))?;

        let arc = Arc::new(computer);
        if let Some(existing) = self.sprites.get(&key) {
            return Ok(existing.clone());
        }
        self.sprites.insert(key, arc.clone());
        Ok(arc)
    }

    pub async fn connect_sprite_computer(
        &self,
        config: &Config,
        pool: &sqlx::PgPool,
        owner_id: &str,
        computer_id: &str,
        browser_enabled: bool,
    ) -> Result<Arc<dyn AgentComputer>, ApiError> {
        Ok(self
            .connect_sprite(config, pool, owner_id, computer_id, browser_enabled)
            .await?)
    }
}
