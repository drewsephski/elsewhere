//! Live Fly Sprites smoke test (manual).
//!
//! Requires:
//! - `SPRITE_TOKEN` or `SPRITES_TOKEN`
//! - `ELSEWHERE_TEST_SPRITE` (reused; not destroyed)
//!
//! Credentials: export vars in your shell, or put them in `.env` at the repo root
//! (loaded automatically; `cargo` does not read `.env` by itself).
//!
//! Run twice to prove persistence across processes:
//! `cargo run -p sprite-computer --example smoke`
//!
//! Browser flows (stateful daemon):
//! `ELSEWHERE_BROWSER_SMOKE=1 cargo run -p sprite-computer --example smoke`
//!
//! For CI-safe checks without Fly credentials, use `agent_cloud_proof` (mock path).

use agent_core::AgentComputer;
use serde_json::{json, Value};
use sprite_computer::{
    default_deny_network_policy, network_policy_matches, SpriteComputer, SpriteComputerConfig,
};
use std::path::PathBuf;
use std::time::Duration;

/// Load repo-root `.env`, overriding stale `export` values in the shell (common when debugging).
fn load_dotenv_files() {
    if dotenvy::dotenv_override().is_ok() {
        return;
    }
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let repo_root: PathBuf = PathBuf::from(manifest).join("../..");
        let _ = dotenvy::from_path_override(repo_root.join(".env"));
    }
}

fn trim_env(value: String) -> String {
    value.trim().to_string()
}

fn sprite_token() -> Option<String> {
    std::env::var("SPRITE_TOKEN")
        .or_else(|_| std::env::var("SPRITES_TOKEN"))
        .ok()
        .map(trim_env)
        .filter(|token| !token.is_empty())
}

fn env_presence_hint() -> String {
    let token = if sprite_token().is_some() {
        "set"
    } else {
        "not set"
    };
    let sprite = if std::env::var("ELSEWHERE_TEST_SPRITE").is_ok() {
        "set"
    } else {
        "not set"
    };
    format!(
        "  SPRITE_TOKEN or SPRITES_TOKEN: {token}\n  ELSEWHERE_TEST_SPRITE: {sprite}\n  SPRITES_API_BASE: {}",
        if std::env::var("SPRITES_API_BASE").is_ok() {
            "set (optional)"
        } else {
            "not set (optional, uses default)"
        }
    )
}

fn load_live_credentials() -> Result<(String, String), String> {
    let token = sprite_token();
    let sprite_name = std::env::var("ELSEWHERE_TEST_SPRITE")
        .ok()
        .map(trim_env)
        .filter(|name| !name.is_empty());

    match (token, sprite_name) {
        (Some(token), Some(sprite_name)) => Ok((token, sprite_name)),
        _ => Err(format!(
            "live smoke needs Fly Sprites credentials in this shell.\n\n\
             Use a repo-root `.env` file or export in this shell:\n\
               SPRITE_TOKEN=...\n\
               ELSEWHERE_TEST_SPRITE=your-sprite-name\n\n\
             Current process:\n{}\n\n\
             Re-run from the repo root: `cargo run -p sprite-computer --example smoke`\n\
             Without Fly access, use: cargo run -p sprite-computer --example agent_cloud_proof",
            env_presence_hint()
        )),
    }
}

const PROOF_PATH: &str = "/workspace/elsewhere-cloud-proof.txt";
const PROOF_CONTENT: &str = "hello from Elsewhere cloud";

fn first_ref_matching(snapshot: &str, tag: &str) -> Option<String> {
    for line in snapshot.lines() {
        if line.contains(&format!("[{tag}]")) {
            if let Some(rest) = line.strip_prefix("- ") {
                if let Some(ref_id) = rest.split_whitespace().next() {
                    return Some(ref_id.to_string());
                }
            }
        }
    }
    first_ref(snapshot)
}

fn first_ref(snapshot: &str) -> Option<String> {
    for line in snapshot.lines() {
        if let Some(rest) = line.strip_prefix("- ") {
            if let Some(ref_id) = rest.split_whitespace().next() {
                if ref_id.starts_with('e') {
                    return Some(ref_id.to_string());
                }
            }
        }
    }
    None
}

fn snapshot_text(value: &Value) -> &str {
    value.get("snapshot").and_then(|v| v.as_str()).unwrap_or("")
}

async fn run_browser_smoke(computer: &SpriteComputer) -> Result<(), Box<dyn std::error::Error>> {
    let navigate = computer
        .browser_invoke("navigate", &json!({ "url": "https://example.com" }))
        .await?;
    println!("browser navigate: {navigate}");
    let start_url = navigate
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    let snapshot = computer.browser_invoke("snapshot", &json!({})).await?;
    println!("browser snapshot: {snapshot}");
    let snap_text = snapshot_text(&snapshot);
    let link_ref = first_ref_matching(snap_text, "a")
        .or_else(|| first_ref(snap_text))
        .ok_or("example.com snapshot missing clickable ref")?;

    let after_click = computer
        .browser_invoke("click", &json!({ "ref": link_ref }))
        .await?;
    println!("browser click: {after_click}");
    let after_url = after_click
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if after_url == start_url {
        return Err("click did not change page URL".into());
    }

    let snapshot2 = computer.browser_invoke("snapshot", &json!({})).await?;
    println!("browser snapshot after click: {snapshot2}");

    computer
        .browser_invoke(
            "navigate",
            &json!({ "url": "https://httpbin.org/forms/post" }),
        )
        .await?;

    let form_snapshot = computer.browser_invoke("snapshot", &json!({})).await?;
    let form_text = snapshot_text(&form_snapshot);
    let input_ref = first_ref_matching(form_text, "input")
        .or_else(|| first_ref(form_text))
        .ok_or("httpbin form snapshot missing input ref")?;

    let typed = "elsewhere-smoke-typed";
    computer
        .browser_invoke(
            "type",
            &json!({ "ref": input_ref, "text": typed, "submit": false }),
        )
        .await?;

    let verify_snapshot = computer.browser_invoke("snapshot", &json!({})).await?;
    let verify_url = verify_snapshot
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if !verify_url.contains("forms/post") {
        return Err("type flow lost form page URL".into());
    }

    // Second type on the same ref proves the daemon session and refs survived.
    computer
        .browser_invoke(
            "type",
            &json!({ "ref": input_ref, "text": "elsewhere-smoke-updated", "submit": false }),
        )
        .await?;
    let after_second_type = computer.browser_invoke("snapshot", &json!({})).await?;
    if after_second_type
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        != verify_url
    {
        return Err("second type changed page URL unexpectedly".into());
    }
    println!("browser type persistence verified");

    let baseline = default_deny_network_policy();
    let policy = computer
        .client()
        .get_network_policy()
        .await
        .map_err(|e| format!("network policy read failed: {e}"))?;
    if !network_policy_matches(&policy, &baseline) {
        return Err("default-deny network policy was not restored after browser smoke".into());
    }
    println!("default-deny network policy verified after browser smoke");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    load_dotenv_files();
    let (token, sprite_name) = load_live_credentials().map_err(|msg| msg)?;
    eprintln!("live smoke: sprite `{sprite_name}` (token loaded)");
    let base_url = std::env::var("SPRITES_API_BASE")
        .unwrap_or_else(|_| sprite_computer::DEFAULT_API_BASE.into());

    let browser_smoke = std::env::var("ELSEWHERE_BROWSER_SMOKE")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let config = SpriteComputerConfig {
        base_url,
        token,
        sprite_name,
        workspace_root: "/workspace".into(),
        request_timeout: Duration::from_secs(120),
        auto_create: true,
        network_policy: default_deny_network_policy(),
        exec_timeout: Duration::from_secs(60),
        browser_enabled: browser_smoke,
        browser_exec_timeout: Duration::from_secs(120),
        browser_profile_host_dir: None,
    };

    let computer = SpriteComputer::new(config)?;
    let info = computer.ensure_ready().await.map_err(|err| {
        let message = err.to_string();
        if message.contains("authentication") {
            return format!(
                "{message}\n\nThe API rejected your Sprites token. \
                 Confirm `SPRITE_TOKEN` in repo-root `.env` (smoke reloads `.env` over shell exports). \
                 Regenerate the token in the Fly Sprites dashboard if needed."
            );
        }
        message
    })?;
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

    if browser_smoke {
        run_browser_smoke(&computer).await?;
    }

    println!("smoke test passed (sprite left running)");
    Ok(())
}
