//! One-shot group routing decision via Codex subscription (tool-less thread).

use crate::error::CodexProviderError;
use crate::toolless_turn::run_toolless_codex_turn;

pub async fn run_codex_group_route_decision(
    executable: Option<std::path::PathBuf>,
    profile: Option<std::path::PathBuf>,
    model: &str,
    developer_instructions: &str,
    user_prompt: &str,
) -> Result<String, CodexProviderError> {
    run_toolless_codex_turn(
        executable,
        profile,
        model,
        developer_instructions,
        user_prompt,
    )
    .await
}
