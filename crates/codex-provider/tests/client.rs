#[tokio::test]
async fn client_initialize_and_account_read_with_fake_server() {
    use codex_provider::{spawn_fake_app_server, CodexAccountKind, CodexAppServerClient};

    let process = spawn_fake_app_server().await.expect("fake server");
    let client = CodexAppServerClient::from_process(process)
        .await
        .expect("client");
    let account = client.account().await.expect("account");
    assert!(matches!(account.account, CodexAccountKind::ChatGpt { .. }));
    client.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn client_lists_mcp_tools_from_fake_server() {
    use codex_provider::{spawn_fake_app_server, CodexAppServerClient};

    let process = spawn_fake_app_server().await.expect("fake server");
    let client = CodexAppServerClient::from_process(process)
        .await
        .expect("client");
    let tools = client
        .list_mcp_server_tools("thread-1")
        .await
        .expect("tools");
    assert!(tools.contains(&"workspace_list".to_string()));
    assert!(tools.contains(&"workspace_exec".to_string()));
    client.shutdown().await.expect("shutdown");
}

#[test]
fn subscription_profile_launch_is_explicit_and_strips_api_key() {
    let launch = codex_provider::CodexProcessLaunch::from_path("/usr/bin/codex".into())
        .subscription_child()
        .with_profile(std::path::Path::new("/private/elsewhere/profile"));
    assert!(launch.strip_openai_api_key);
    assert_eq!(
        launch.env.get("CODEX_HOME").unwrap(),
        "/private/elsewhere/profile"
    );
    assert!(launch
        .config_overrides
        .iter()
        .any(|value| value == "cli_auth_credentials_store=\"file\""));
}
