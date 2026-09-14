//! Live Fly Sprites smoke test (manual).
//!
//! Requires:
//! - `SPRITES_TOKEN`
//! - `ELSEWHERE_TEST_SPRITE` (reused; not destroyed)
//!
//! Run twice to prove persistence across processes:
//! `cargo run -p sprite-computer --example smoke`

use agent_core::AgentComputer;
use sprite_computer::{
    default_deny_network_policy, SpriteComputer, SpriteComputerConfig,
};
use std::time::Duration;

const PROOF_PATH: &str = "/workspace/elsewhere-cloud-proof.txt";
const PROOF_CONTENT: &str = "hello from Elsewhere cloud";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = std::env::var("SPRITES_TOKEN")?;
    let sprite_name = std::env::var("ELSEWHERE_TEST_SPRITE")?;
    let base_url = std::env::var("SPRITES_API_BASE")
        .unwrap_or_else(|_| sprite_computer::DEFAULT_API_BASE.into());

    let config = SpriteComputerConfig {
        base_url,
        token,
        sprite_name,
        workspace_root: "/workspace".into(),
        request_timeout: Duration::from_secs(120),
        auto_create: true,
        network_policy: default_deny_network_policy(),
        exec_timeout: Duration::from_secs(60),
    };

    let computer = SpriteComputer::new(config)?;
    let info = computer.ensure_ready().await?;
    println!("sprite ready: {:?}", info.detail);

    computer
        .write_file(PROOF_PATH, PROOF_CONTENT.as_bytes())
        .await?;
    let read_back = computer.read_file(PROOF_PATH).await?;
    assert_eq!(String::from_utf8_lossy(&read_back), PROOF_CONTENT);

    let listing = computer.list_dir("/workspace").await?;
    println!("workspace entries: {}", listing.len());

    let pwd = computer.exec("pwd").await?;
    assert!(
        pwd.stdout.trim() == "/workspace",
        "expected /workspace, got {:?}",
        pwd.stdout
    );

    let cat = computer.exec(&format!("cat {PROOF_PATH}")).await?;
    assert_eq!(cat.stdout.trim(), PROOF_CONTENT);

    let checkpoint = computer
        .create_checkpoint(Some("Elsewhere Phase 3A proof"))
        .await?;
    println!("checkpoint created: {}", checkpoint.id);

    println!("smoke test passed (sprite left running)");
    Ok(())
}
