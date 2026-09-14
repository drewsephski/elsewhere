#[tokio::test]
async fn probe_reports_chatgpt_available_on_fake_server() {
    use codex_provider::{
        probe_codex_subscription_availability_on_client, spawn_fake_app_server,
        CodexAppServerClient, CodexSubscriptionAvailability,
    };

    let process = spawn_fake_app_server().await.expect("fake server");
    let client = CodexAppServerClient::from_process(process)
        .await
        .expect("client");
    let availability = probe_codex_subscription_availability_on_client(&client).await;
    assert!(matches!(
        availability,
        CodexSubscriptionAvailability::Available { plan_type: Some(_) }
    ));
    client.shutdown().await.expect("shutdown");
}
