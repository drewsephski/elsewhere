use std::sync::Arc;

use agent_core::AgentComputer;

use crate::app_state::AppState;
use crate::error::ApiError;

pub async fn connect_sprite_computer(
    state: &AppState,
    owner_id: &str,
    computer_id: &str,
) -> Result<Arc<dyn AgentComputer>, ApiError> {
    if !state.config.browser_enabled {
        return Err(ApiError::Validation(
            "browser preview is disabled on this host".into(),
        ));
    }
    state
        .computer_registry
        .connect_sprite_computer(
            &state.config,
            &state.pool,
            owner_id,
            computer_id,
            true,
        )
        .await
}
