use agent_core::{AgentConnectors, ConnectorError};
use base64::Engine;
use chrono::{Duration, Utc};
use cloud_host::connectors::{
    db::{
        consume_oauth_state, disconnect, get_for_owner, load_access_token, store_oauth_state,
        upsert_connected, PROVIDER_GITHUB,
    },
    secret::ConnectorSecretBox,
    service::PostgresAgentConnectors,
    GitHubClient,
};
use serde_json::json;
use sqlx::PgPool;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_secret_box() -> ConnectorSecretBox {
    let key = base64::engine::general_purpose::STANDARD.encode([7u8; 32]);
    ConnectorSecretBox::from_base64_key(&key).expect("test key")
}

#[sqlx::test(migrations = "./migrations")]
async fn connector_ownership_isolated_in_database(pool: PgPool) {
    let secret = test_secret_box();
    let metadata = json!({ "login": "alice" });
    upsert_connected(
        &pool,
        "alice",
        PROVIDER_GITHUB,
        &metadata,
        "gho_alice_token",
        &secret,
    )
    .await
    .unwrap();

    let bob = get_for_owner(&pool, "bob", PROVIDER_GITHUB).await.unwrap();
    assert!(bob.is_none());

    let alice = get_for_owner(&pool, "alice", PROVIDER_GITHUB)
        .await
        .unwrap();
    assert_eq!(alice.unwrap().status, "connected");
}

#[sqlx::test(migrations = "./migrations")]
async fn disconnected_github_tool_returns_not_connected(pool: PgPool) {
    let secret = test_secret_box();
    let connectors =
        PostgresAgentConnectors::new(pool.clone(), secret.into(), GitHubClient::production());
    let err = connectors
        .dispatch_connector_tool("nobody", "github_list_repositories", &json!({}))
        .await
        .unwrap_err();
    assert_eq!(err, ConnectorError::NotConnected);
}

#[sqlx::test(migrations = "./migrations")]
async fn connector_secret_never_appears_in_connector_metadata(pool: PgPool) {
    let secret = test_secret_box();
    let token = "gho_super_secret_token_value";
    upsert_connected(
        &pool,
        "alice",
        PROVIDER_GITHUB,
        &json!({ "login": "alice" }),
        token,
        &secret,
    )
    .await
    .unwrap();

    let row = get_for_owner(&pool, "alice", PROVIDER_GITHUB)
        .await
        .unwrap()
        .unwrap();
    let serialized = row.metadata.to_string();
    assert!(!serialized.contains(token));
    assert!(!serialized.contains("gho_"));
}

#[tokio::test]
async fn redact_strips_github_tokens_from_errors() {
    let token = "gho_abcdefghijklmnopqrstuvwxyz1234567890";
    let redacted = cloud_host::redact::redact_secrets(&format!("failed: {token}"));
    assert!(!redacted.contains(token));
}

#[sqlx::test(migrations = "./migrations")]
async fn github_disconnect_clears_encrypted_secret(pool: PgPool) {
    let secret = test_secret_box();
    upsert_connected(
        &pool,
        "alice",
        PROVIDER_GITHUB,
        &json!({ "login": "alice" }),
        "gho_disconnect_test",
        &secret,
    )
    .await
    .unwrap();

    assert!(load_access_token(&pool, "alice", PROVIDER_GITHUB, &secret)
        .await
        .unwrap()
        .is_some());

    assert!(disconnect(&pool, "alice", PROVIDER_GITHUB).await.unwrap());

    assert!(load_access_token(&pool, "alice", PROVIDER_GITHUB, &secret)
        .await
        .unwrap()
        .is_none());
    let row = get_for_owner(&pool, "alice", PROVIDER_GITHUB)
        .await
        .unwrap()
        .expect("connector row");
    assert_eq!(row.status, "disconnected");
}

#[sqlx::test(migrations = "./migrations")]
async fn oauth_state_consumption_is_owner_scoped(pool: PgPool) {
    let state = "oauth-state-alice-only";
    let expires_at = Utc::now() + Duration::minutes(10);
    store_oauth_state(&pool, state, "alice", PROVIDER_GITHUB, expires_at)
        .await
        .unwrap();

    assert!(!consume_oauth_state(&pool, state, PROVIDER_GITHUB, "bob")
        .await
        .unwrap());

    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::bigint FROM connector_oauth_states WHERE state = $1")
            .bind(state)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(remaining, 1);

    assert!(consume_oauth_state(&pool, state, PROVIDER_GITHUB, "alice")
        .await
        .unwrap());
}

#[sqlx::test(migrations = "./migrations")]
async fn github_list_repositories_dispatches_via_connector_service(pool: PgPool) {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user/repos"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "full_name": "octocat/Hello-World", "private": false }
        ])))
        .mount(&mock)
        .await;

    let secret = test_secret_box();
    upsert_connected(
        &pool,
        "alice",
        PROVIDER_GITHUB,
        &json!({ "login": "alice" }),
        "gho_test_token",
        &secret,
    )
    .await
    .unwrap();

    let api_base = mock.uri();
    let client = GitHubClient::with_api_base(api_base.clone(), api_base);
    let connectors = PostgresAgentConnectors::new(pool.clone(), secret.into(), client);

    let value = connectors
        .dispatch_connector_tool("alice", "github_list_repositories", &json!({}))
        .await
        .expect("dispatch");
    assert_eq!(value.get("ok"), Some(&json!(true)));
    let repos = value
        .get("repositories")
        .and_then(|v| v.as_array())
        .unwrap();
    assert_eq!(repos.len(), 1);
}
