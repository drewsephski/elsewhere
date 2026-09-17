//! Shared `SpriteComputer` instances per owner/computer so execution gates cannot be bypassed.

use std::sync::Arc;
use std::time::{Duration, Instant};

use agent_core::AgentComputer;
use dashmap::DashMap;
use sprite_computer::{default_deny_network_policy, SpriteComputer, SpriteComputerConfig};

use crate::config::Config;
use crate::error::ApiError;
use crate::local_mac::session::{LocalMacSessionRegistry, RemoteLocalMacComputer};
use crate::runner::sprite_resource_for_computer;

const MAX_CACHED_COMPUTERS: usize = 128;
const IDLE_EVICTION: Duration = Duration::from_secs(30 * 60);

#[derive(Clone)]
struct CachedEntry {
    computer: Arc<SpriteComputer>,
    sprite_name: String,
    browser_enabled: bool,
    last_used: Instant,
}

#[derive(Clone, Default)]
pub struct ComputerRegistry {
    sprites: Arc<DashMap<(String, String), CachedEntry>>,
}

impl ComputerRegistry {
    pub fn evict(&self, owner_id: &str, computer_id: &str) {
        self.sprites
            .remove(&(owner_id.to_string(), computer_id.to_string()));
    }

    fn evict_idle_and_bound(&self) {
        let now = Instant::now();
        let mut stale_keys: Vec<(String, String)> = Vec::new();
        for entry in self.sprites.iter() {
            if now.duration_since(entry.value().last_used) > IDLE_EVICTION {
                stale_keys.push(entry.key().clone());
            }
        }
        for key in stale_keys {
            self.sprites.remove(&key);
        }
        while self.sprites.len() > MAX_CACHED_COMPUTERS {
            let oldest = self
                .sprites
                .iter()
                .min_by_key(|entry| entry.value().last_used)
                .map(|entry| entry.key().clone());
            if let Some(key) = oldest {
                self.sprites.remove(&key);
            } else {
                break;
            }
        }
    }

    async fn verify_active_computer(
        pool: &sqlx::PgPool,
        owner_id: &str,
        computer_id: &str,
    ) -> Result<(), ApiError> {
        let provider: Option<String> = sqlx::query_scalar(
            "SELECT provider FROM sandboxes
             WHERE id = $1 AND owner_id = $2 AND state <> 'archived'",
        )
        .bind(computer_id)
        .bind(owner_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
        match provider.as_deref() {
            None => Err(ApiError::NotFound),
            Some(crate::local_mac::PROVIDER) => Err(ApiError::Validation(
                "This Mac is not available for hosted Sprite operations yet.".into(),
            )),
            Some(_) => Ok(()),
        }
    }

    fn build_sprite(
        config: &Config,
        sprite_name: String,
        browser_enabled: bool,
        browser_profile_host_dir: Option<std::path::PathBuf>,
    ) -> Result<Arc<SpriteComputer>, ApiError> {
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
            browser_profile_host_dir,
        })
        .map_err(|e| ApiError::Internal(format!("SpriteComputer: {e}")))?;
        Ok(Arc::new(computer))
    }

    pub async fn connect_sprite(
        &self,
        config: &Config,
        pool: &sqlx::PgPool,
        owner_id: &str,
        computer_id: &str,
        browser_enabled: bool,
    ) -> Result<Arc<SpriteComputer>, ApiError> {
        Self::verify_active_computer(pool, owner_id, computer_id).await?;

        let sprite_name = sprite_resource_for_computer(pool, computer_id)
            .await
            .map_err(ApiError::Internal)?;

        let key = (owner_id.to_string(), computer_id.to_string());

        // Never call `get_mut` while a `get` guard is live on the same key — DashMap will deadlock.
        if let Some(mut entry) = self.sprites.get_mut(&key) {
            if entry.sprite_name == sprite_name && entry.browser_enabled == browser_enabled {
                entry.last_used = Instant::now();
                return Ok(entry.computer.clone());
            }
        }
        self.sprites.remove(&key);

        self.evict_idle_and_bound();

        let browser_profile_host_dir = if browser_enabled && config.browser_profiles_dir.is_some() {
            Some(
                crate::browser_profile::profile_for_computer(pool, config, owner_id, computer_id)
                    .await?,
            )
        } else {
            None
        };

        let computer = Self::build_sprite(
            config,
            sprite_name.clone(),
            browser_enabled,
            browser_profile_host_dir,
        )?;

        let entry = CachedEntry {
            computer: computer.clone(),
            sprite_name,
            browser_enabled,
            last_used: Instant::now(),
        };

        if let Some(mut hit) = self.sprites.get_mut(&key) {
            if hit.sprite_name == entry.sprite_name && hit.browser_enabled == entry.browser_enabled
            {
                hit.last_used = Instant::now();
                return Ok(hit.computer.clone());
            }
            self.sprites.remove(&key);
        }
        self.sprites.insert(key, entry);
        Ok(computer)
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

    /// Provider-neutral computer resolver. Bot runs still use `connect_sprite`
    /// via `runner::build_computer`; this seam is for the next local-Mac slice.
    pub async fn connect_agent_computer(
        &self,
        config: &Config,
        pool: &sqlx::PgPool,
        owner_id: &str,
        computer_id: &str,
        local_mac_sessions: &LocalMacSessionRegistry,
        browser_enabled: bool,
    ) -> Result<Arc<dyn AgentComputer>, ApiError> {
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT provider, provider_resource_id FROM sandboxes
             WHERE id = $1 AND owner_id = $2 AND state <> 'archived'",
        )
        .bind(computer_id)
        .bind(owner_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
        let Some((provider, resource_id)) = row else {
            return Err(ApiError::NotFound);
        };
        match provider.as_str() {
            "fly_sprite" => {
                self.connect_sprite_computer(config, pool, owner_id, computer_id, browser_enabled)
                    .await
            }
            crate::local_mac::PROVIDER => Ok(Arc::new(RemoteLocalMacComputer::new(
                owner_id,
                computer_id,
                resource_id,
                local_mac_sessions.clone(),
            ))),
            other => Err(ApiError::Validation(format!(
                "unsupported computer provider: {other}"
            ))),
        }
    }
}
