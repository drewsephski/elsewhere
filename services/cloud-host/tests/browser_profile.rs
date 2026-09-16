//! Durable browser profile mapping and cookie bundle roundtrip.

use cloud_host::config::{AuthMode, Config};
use cloud_host::db::resources::insert_computer_placeholder;
use cloud_host::run_engine_select::RunEngineMode;
use sqlx::PgPool;
use std::io::Write;
use std::time::Duration;
use uuid::Uuid;

fn test_config(browser_profiles_dir: std::path::PathBuf) -> Config {
    Config {
        database_url: std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://elsewhere:elsewhere@127.0.0.1:5432/elsewhere".into()),
        openai_api_key: Some("test-key".into()),
        sprite_token: "test-sprite".into(),
        api_token: "test-token".into(),
        auth_mode: AuthMode::Jwt,
        jwt_issuer: Some("http://localhost:3000".into()),
        jwt_audience: Some("elsewhere-cloud-host".into()),
        jwt_jwks_url: Some("http://127.0.0.1:9/jwks".into()),
        cors_web_origin: None,
        allow_codex_login: false,
        sprites_api_base: "http://127.0.0.1:9".into(),
        max_concurrent_runs: 2,
        run_timeout_secs: 120,
        bind_addr: "127.0.0.1:0".into(),
        run_engine: RunEngineMode::Responses,
        codex_executable: None,
        codex_profiles_dir: None,
        browser_profiles_dir: Some(browser_profiles_dir),
        tool_approval_timeout_secs: 300,
        enforce_tool_approvals_internal: false,
        legacy_local_approval_bypass: false,
        browser_enabled: true,
        connector_secret_key: None,
        github_client_id: None,
        github_client_secret: None,
        github_oauth_redirect_uri: None,
    }
}

async fn try_test_pool() -> Option<PgPool> {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://elsewhere:elsewhere@127.0.0.1:5432/elsewhere".into());
    let pool = tokio::time::timeout(Duration::from_secs(2), PgPool::connect(&url))
        .await
        .ok()?
        .ok()?;
    sqlx::migrate!("./migrations").run(&pool).await.ok()?;
    Some(pool)
}

#[tokio::test]
async fn browser_profiles_are_persistent_per_computer_and_owner_scoped() {
    let pool = try_test_pool()
        .await
        .expect("test Postgres must be running");
    let root = std::env::temp_dir().join(format!("elsewhere-browser-profiles-{}", Uuid::new_v4()));
    let config = test_config(root.clone());
    let owner_a = format!("owner-a-{}", Uuid::new_v4());
    let owner_b = format!("owner-b-{}", Uuid::new_v4());
    let computer_a = insert_computer_placeholder(&pool, &owner_a, "a")
        .await
        .unwrap();
    let computer_b = insert_computer_placeholder(&pool, &owner_b, "b")
        .await
        .unwrap();

    let path_a1 =
        cloud_host::browser_profile::profile_for_computer(&pool, &config, &owner_a, &computer_a.id)
            .await
            .unwrap();
    let path_a2 =
        cloud_host::browser_profile::profile_for_computer(&pool, &config, &owner_a, &computer_a.id)
            .await
            .unwrap();
    let path_b =
        cloud_host::browser_profile::profile_for_computer(&pool, &config, &owner_b, &computer_b.id)
            .await
            .unwrap();

    assert_eq!(path_a1, path_a2);
    assert_ne!(path_a1, path_b);
    assert_eq!(path_a1.parent(), Some(root.as_path()));

    cloud_host::browser_profile::reset_profile_for_computer(
        &pool,
        &config,
        &owner_a,
        &computer_a.id,
    )
    .await
    .unwrap();
    let path_a3 =
        cloud_host::browser_profile::profile_for_computer(&pool, &config, &owner_a, &computer_a.id)
            .await
            .unwrap();
    assert_ne!(path_a1, path_a3);

    std::fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn sequential_host_profile_roundtrip_preserves_session_marker() {
    let temp = std::env::temp_dir().join(format!("elsewhere-browser-session-{}", Uuid::new_v4()));
    let host_profile = temp.join("profile");
    std::fs::create_dir_all(host_profile.join("Default")).unwrap();
    let marker_path = host_profile.join("Default/.session-marker");
    std::fs::File::create(&marker_path)
        .unwrap()
        .write_all(b"cookie=session-abc")
        .unwrap();

    let bundle = std::process::Command::new("tar")
        .args(["-czf", "-", "-C", host_profile.to_str().unwrap(), "."])
        .output()
        .unwrap();
    assert!(bundle.status.success());

    let restored = temp.join("restored");
    std::fs::create_dir_all(&restored).unwrap();
    let mut child = std::process::Command::new("tar")
        .args(["-xzf", "-", "-C", restored.to_str().unwrap()])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(&bundle.stdout)
        .unwrap();
    assert!(child.wait().unwrap().success());

    let first = std::fs::read_to_string(restored.join("Default/.session-marker")).unwrap();
    assert_eq!(first, "cookie=session-abc");

    // Simulate a second browser session: mutate guest state, capture bundle, extract to host again.
    std::fs::write(
        restored.join("Default/.session-marker"),
        b"cookie=session-abc; second=visit",
    )
    .unwrap();
    let bundle2 = std::process::Command::new("tar")
        .args(["-czf", "-", "-C", restored.to_str().unwrap(), "."])
        .output()
        .unwrap();
    let host_after = temp.join("host-after");
    std::fs::create_dir_all(&host_after).unwrap();
    let mut child2 = std::process::Command::new("tar")
        .args(["-xzf", "-", "-C", host_after.to_str().unwrap()])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child2
        .stdin
        .as_mut()
        .unwrap()
        .write_all(&bundle2.stdout)
        .unwrap();
    assert!(child2.wait().unwrap().success());

    let second = std::fs::read_to_string(host_after.join("Default/.session-marker")).unwrap();
    assert!(second.contains("second=visit"));

    std::fs::remove_dir_all(temp).ok();
}
