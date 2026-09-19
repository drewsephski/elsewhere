use agent_core::{AgentConnectors, ConnectorError};
use base64::Engine;
use chrono::{Duration, Utc};
use cloud_host::connectors::{
    db::{
        consume_oauth_state, decrypt_connector_secret, disconnect, get_for_owner,
        load_access_token, load_github_credential, store_oauth_state, upsert_connected,
        upsert_github_app_credential, GitHubCredentialLoad, PROVIDER_GITHUB,
    },
    github_client::GitHubCredential,
    secret::ConnectorSecretBox,
    service::PostgresAgentConnectors,
    GitHubClient,
};
use cloud_host::{build_router, AppState};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[path = "support/mod.rs"]
mod support;
use support::test_config;

fn test_secret_box() -> ConnectorSecretBox {
    let key = base64::engine::general_purpose::STANDARD.encode([7u8; 32]);
    ConnectorSecretBox::from_base64_key(&key).expect("test key")
}

fn test_secret_key() -> String {
    base64::engine::general_purpose::STANDARD.encode([7u8; 32])
}

fn app_credential(access: &str, refresh: &str, access_secs: i64) -> GitHubCredential {
    GitHubCredential {
        kind: "github_app_user".into(),
        access_token: access.into(),
        refresh_token: refresh.into(),
        access_expires_at: Utc::now() + Duration::seconds(access_secs),
        refresh_expires_at: Utc::now() + Duration::days(180),
        token_type: "bearer".into(),
    }
}

fn github_http_config() -> cloud_host::Config {
    let mut config = test_config();
    config.connector_secret_key = Some(test_secret_key());
    config.github_client_id = Some("Iv23lihP3j9Z6kZU0s9p".into());
    config.github_client_secret = Some("github-app-client-secret".into());
    config.github_oauth_redirect_uri =
        Some("http://localhost:3000/app/connectors/github/callback".into());
    config.github_app_slug = Some("elsewhere-alpha".into());
    config
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
    let access = "ghu_abcdefghijklmnopqrstuvwxyz1234567890";
    let refresh = "ghr_refresh_token_material_here";
    let redacted = cloud_host::redact::redact_secrets(&format!("failed: {access} {refresh}"));
    assert!(!redacted.contains(access));
    assert!(!redacted.contains(refresh));
    let provider = ConnectorError::Provider(format!("GitHub API error 401: {access} {refresh}"));
    let loggable = cloud_host::redact::redact_secrets(&provider.message());
    assert!(!loggable.contains(access));
    assert!(!loggable.contains(refresh));
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
    store_oauth_state(&pool, state, "alice", PROVIDER_GITHUB, expires_at, None)
        .await
        .unwrap();

    assert!(consume_oauth_state(&pool, state, PROVIDER_GITHUB, "bob")
        .await
        .unwrap()
        .is_none());

    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::bigint FROM connector_oauth_states WHERE state = $1")
            .bind(state)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(remaining, 1);

    assert!(consume_oauth_state(&pool, state, PROVIDER_GITHUB, "alice")
        .await
        .unwrap()
        .is_some());
}

#[sqlx::test(migrations = "./migrations")]
async fn github_list_repositories_dispatches_via_connector_service(pool: PgPool) {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user/installations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total_count": 1,
            "installations": [{
                "id": 11,
                "account": { "login": "octocat", "id": 1, "type": "User" },
                "repository_selection": "selected",
                "permissions": { "contents": "write" }
            }]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/user/installations/11/repositories"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total_count": 1,
            "repositories": [
                { "full_name": "octocat/Hello-World", "name": "Hello-World", "private": false, "owner": { "login": "octocat" }, "description": "demo" }
            ]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/user/repos"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "full_name": "should-not-appear/from-user-repos" }
        ])))
        .expect(0)
        .mount(&mock)
        .await;

    let secret = test_secret_box();
    upsert_github_app_credential(
        &pool,
        "alice",
        &json!({ "githubUser": { "login": "alice" } }),
        &app_credential("ghu_test_token", "ghr_test_refresh", 8 * 3600),
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
    assert_eq!(
        repos[0].get("full_name"),
        Some(&json!("octocat/Hello-World"))
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn installed_connector_catalog_is_owner_scoped_and_secret_safe(pool: PgPool) {
    use cloud_host::connectors::installs::{
        insert_install, list_installs, list_tools, InstallToolDraft, StoredSecret,
    };
    use cloud_host::connectors::remote::{RemoteHttpClient, RemotePolicy};

    let secret = test_secret_box();
    let token = "mcp-static-secret-token-value";
    let alice = insert_install(
        &pool,
        "alice",
        "mcp",
        "Docs",
        "https://mcp.example.com/mcp",
        &json!({ "auth": "bearer" }),
        "connected",
        Some(&StoredSecret::bearer(token)),
        &secret,
        &[
            InstallToolDraft {
                remote_name: "search_docs".into(),
                display_name: "search_docs".into(),
                description: "Search docs".into(),
                input_schema: json!({ "type": "object" }),
                read_only: true,
            },
            InstallToolDraft {
                remote_name: "create_doc".into(),
                display_name: "create_doc".into(),
                description: "Create a doc".into(),
                input_schema: json!({ "type": "object" }),
                read_only: false,
            },
        ],
    )
    .await
    .unwrap();
    let bob = insert_install(
        &pool,
        "bob",
        "mcp",
        "Docs",
        "https://mcp.example.net/mcp",
        &json!({ "auth": "none" }),
        "connected",
        None,
        &secret,
        &[InstallToolDraft {
            remote_name: "search_docs".into(),
            display_name: "search_docs".into(),
            description: "Bob search".into(),
            input_schema: json!({ "type": "object" }),
            read_only: true,
        }],
    )
    .await
    .unwrap();

    let alice_listed = list_installs(&pool, "alice").await.unwrap();
    assert_eq!(alice_listed.len(), 1);
    assert_eq!(alice_listed[0].id, alice.id);
    assert!(!format!("{alice_listed:?}").contains(token));
    assert!(list_installs(&pool, "bob").await.unwrap()[0].id == bob.id);

    let connectors = PostgresAgentConnectors::new_with_remote(
        pool.clone(),
        secret.into(),
        GitHubClient::production(),
        RemoteHttpClient::new(RemotePolicy::for_tests()),
    );
    let alice_search = connectors
        .search_connected_app_tools("alice", None, None, None)
        .await
        .unwrap();
    let alice_tools = alice_search
        .get("tools")
        .and_then(|v| v.as_array())
        .unwrap();
    assert_eq!(alice_tools.len(), 2);
    let alice_ids: Vec<_> = alice_tools
        .iter()
        .map(|t| {
            t.get("toolId")
                .and_then(|v| v.as_str())
                .unwrap()
                .to_string()
        })
        .collect();

    let bob_search = connectors
        .search_connected_app_tools("bob", None, None, None)
        .await
        .unwrap();
    let bob_tools = bob_search.get("tools").and_then(|v| v.as_array()).unwrap();
    assert_eq!(bob_tools.len(), 1);
    let bob_id = bob_tools[0].get("toolId").and_then(|v| v.as_str()).unwrap();
    assert!(!alice_ids.iter().any(|id| id == bob_id));

    let err = connectors
        .execute_connected_app_tool("alice", bob_id, &json!({}))
        .await
        .unwrap_err();
    assert_eq!(err, ConnectorError::NotFound);

    let invalid = connectors
        .load_connected_app_tool("alice", "not-a-uuid")
        .await
        .unwrap_err();
    assert_eq!(invalid, ConnectorError::NotFound);

    let tools = list_tools(&pool, alice.id).await.unwrap();
    let loaded = connectors
        .load_connected_app_tool("alice", &tools[0].id.to_string())
        .await
        .unwrap();
    assert_eq!(loaded.source, "Docs");
    assert!(!loaded.input_schema.to_string().contains(token));
}

#[sqlx::test(migrations = "./migrations")]
async fn mcp_oauth_session_cannot_be_consumed_by_another_owner(pool: PgPool) {
    use cloud_host::connectors::mcp_oauth::{complete_mcp_oauth, PendingOAuthConfig};
    use cloud_host::connectors::remote::{RemoteHttpClient, RemotePolicy};

    let secret = test_secret_box();
    let (nonce, ciphertext) = secret.encrypt("pkce-verifier").unwrap();
    let state = "oauth-state-mcp-alice";
    sqlx::query(
        r#"
        INSERT INTO connector_mcp_oauth_sessions (
            state, owner_id, install_id, code_verifier_nonce, code_verifier_ciphertext,
            pending_config, expires_at
        )
        VALUES ($1, $2, NULL, $3, $4, $5, NOW() + INTERVAL '10 minutes')
        "#,
    )
    .bind(state)
    .bind("alice")
    .bind(nonce)
    .bind(ciphertext)
    .bind(json!(PendingOAuthConfig {
        display_name: "Docs".into(),
        endpoint_url: "https://mcp.example.com/mcp".into(),
        kind: "mcp".into(),
        redirect_uri: "https://app.example/app/connectors/mcp/callback".into(),
        client_id: Some("client".into()),
        authorization_endpoint: Some("https://auth.example/authorize".into()),
        token_endpoint: Some("https://auth.example/token".into()),
        resource: None,
    }))
    .execute(&pool)
    .await
    .unwrap();

    let err = complete_mcp_oauth(
        &pool,
        &secret,
        &RemoteHttpClient::new(RemotePolicy::for_tests()),
        "bob",
        "code",
        state,
    )
    .await
    .unwrap_err();
    assert!(
        err.to_string().contains("invalid")
            || err.to_string().contains("expired")
            || matches!(err, cloud_host::error::ApiError::Validation(_))
    );

    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM connector_mcp_oauth_sessions WHERE state = $1",
    )
    .bind(state)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, 1);
}

fn github_client(mock: &MockServer) -> GitHubClient {
    GitHubClient::with_api_base(mock.uri(), mock.uri()).with_oauth(
        "Iv23lihP3j9Z6kZU0s9p".into(),
        "github-app-client-secret".into(),
    )
}

async fn json_auth(
    app: axum::Router,
    method: &str,
    uri: &str,
    body: Value,
) -> (http::StatusCode, Value) {
    let response = app
        .oneshot(
            http::Request::builder()
                .method(method)
                .uri(uri)
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(json!({}));
    (status, json)
}

fn installation_json(id: i64, login: &str, account_type: &str) -> Value {
    json!({
        "id": id,
        "account": { "login": login, "id": id, "type": account_type },
        "repository_selection": "selected",
        "permissions": { "contents": "write", "issues": "read", "pull_requests": "write" }
    })
}

fn repo_json(full_name: &str, private: bool, description: &str) -> Value {
    let name = full_name.split('/').nth(1).unwrap_or(full_name);
    let owner = full_name.split('/').next().unwrap_or(full_name);
    json!({
        "full_name": full_name,
        "name": name,
        "private": private,
        "description": description,
        "owner": { "login": owner }
    })
}

async fn mock_token_exchange(mock: &MockServer, access: &str, refresh: &str) {
    Mock::given(method("POST"))
        .and(path("/login/oauth/access_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": access,
            "expires_in": 28800,
            "refresh_token": refresh,
            "refresh_token_expires_in": 15897600,
            "token_type": "bearer",
            "scope": ""
        })))
        .mount(mock)
        .await;
}

async fn mock_user_and_installs(
    mock: &MockServer,
    installs: &[(i64, &str, &str)],
    repos: &[(i64, Vec<Value>)],
) {
    Mock::given(method("GET"))
        .and(path("/user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "login": "octocat",
            "id": 1,
            "name": "The Octocat",
            "avatar_url": "https://github.com/octocat.png"
        })))
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/user/installations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total_count": installs.len(),
            "installations": installs.iter().map(|(id, login, ty)| installation_json(*id, login, ty)).collect::<Vec<_>>()
        })))
        .mount(mock)
        .await;
    for (id, repositories) in repos {
        Mock::given(method("GET"))
            .and(path(format!("/user/installations/{id}/repositories")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "total_count": repositories.len(),
                "repositories": repositories
            })))
            .mount(mock)
            .await;
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn github_app_install_start_uses_slug_and_owner_bound_state(pool: PgPool) {
    let mock = MockServer::start().await;
    let mut state = AppState::new(pool.clone(), github_http_config());
    state.github_client = github_client(&mock);
    let app = build_router(state);

    let (status, first) = json_auth(
        app.clone(),
        "POST",
        "/v1/connectors/github/oauth/start",
        json!({}),
    )
    .await;
    assert_eq!(status, http::StatusCode::OK);
    let url = first["authorizeUrl"].as_str().unwrap();
    assert!(url.contains("/apps/elsewhere-alpha/installations/new"));
    assert!(url.contains("state="));
    assert!(!url.contains("scope="));
    assert!(!url.contains("repo"));
    assert!(!url.contains("/login/oauth/authorize"));
    let state_a = first["state"].as_str().unwrap().to_string();
    assert_eq!(state_a.len(), 36);

    let (status, second) = json_auth(
        app.clone(),
        "POST",
        "/v1/connectors/github/oauth/start",
        json!({}),
    )
    .await;
    assert_eq!(status, http::StatusCode::OK);
    assert_ne!(second["state"].as_str().unwrap(), state_a);

    assert!(consume_oauth_state(&pool, &state_a, PROVIDER_GITHUB, "bob")
        .await
        .unwrap()
        .is_none());
    assert!(
        consume_oauth_state(&pool, &state_a, PROVIDER_GITHUB, "legacy-local")
            .await
            .unwrap()
            .is_some()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn github_app_exchange_stores_structured_secret_and_installations(pool: PgPool) {
    let mock = MockServer::start().await;
    mock_token_exchange(&mock, "ghu_access_live", "ghr_refresh_live").await;
    mock_user_and_installs(
        &mock,
        &[(11, "octocat", "User"), (22, "acme", "Organization")],
        &[
            (11, vec![repo_json("octocat/Hello-World", false, "demo")]),
            (22, vec![repo_json("acme/private", true, "secret work")]),
        ],
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/user/installations/999/repositories"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "repositories": [repo_json("evil/widen", true, "nope")]
        })))
        .expect(0)
        .mount(&mock)
        .await;

    let mut state = AppState::new(pool.clone(), github_http_config());
    state.github_client = github_client(&mock);
    let app = build_router(state.clone());

    let (status, start) = json_auth(
        app.clone(),
        "POST",
        "/v1/connectors/github/oauth/start",
        json!({}),
    )
    .await;
    assert_eq!(status, http::StatusCode::OK);
    let oauth_state = start["state"].as_str().unwrap();

    let (status, connected) = json_auth(
        app.clone(),
        "POST",
        "/v1/connectors/github/oauth/complete",
        json!({
            "code": "install-code",
            "state": oauth_state,
            "installationId": 999
        }),
    )
    .await;
    assert_eq!(status, http::StatusCode::OK);
    assert_eq!(connected["status"], "connected");
    let body = connected.to_string();
    assert!(!body.contains("ghu_"));
    assert!(!body.contains("ghr_"));
    assert!(!body.contains("github-app-client-secret"));
    let metadata = &connected["metadata"];
    assert_eq!(metadata["githubUser"]["login"], "octocat");
    assert_eq!(metadata["authorizedRepositoryCount"], 2);
    let installs = metadata["installations"].as_array().unwrap();
    assert_eq!(installs.len(), 2);
    assert!(installs.iter().any(|row| row["id"] == 11));
    assert!(installs.iter().any(|row| row["id"] == 22));
    assert!(!installs.iter().any(|row| row["id"] == 999));

    let secret = test_secret_box();
    let plaintext = decrypt_connector_secret(&pool, "legacy-local", PROVIDER_GITHUB, &secret)
        .await
        .unwrap()
        .unwrap();
    assert!(plaintext.contains("ghu_access_live"));
    assert!(plaintext.contains("ghr_refresh_live"));
    assert!(GitHubCredential::from_plaintext(&plaintext).is_some());
    assert!(!metadata.to_string().contains("ghu_access_live"));
    assert!(!metadata.to_string().contains("ghr_refresh_live"));
}

#[sqlx::test(migrations = "./migrations")]
async fn github_app_complete_rejects_zero_installations(pool: PgPool) {
    let mock = MockServer::start().await;
    mock_token_exchange(&mock, "ghu_access_none", "ghr_refresh_none").await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "login": "octocat",
            "id": 1
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/user/installations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total_count": 0,
            "installations": []
        })))
        .mount(&mock)
        .await;

    let mut state = AppState::new(pool.clone(), github_http_config());
    state.github_client = github_client(&mock);
    let app = build_router(state);
    let (status, start) = json_auth(
        app.clone(),
        "POST",
        "/v1/connectors/github/oauth/start",
        json!({}),
    )
    .await;
    assert_eq!(status, http::StatusCode::OK);
    let (status, body) = json_auth(
        app,
        "POST",
        "/v1/connectors/github/oauth/complete",
        json!({ "code": "x", "state": start["state"] }),
    )
    .await;
    assert_eq!(status, http::StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("not installed"));
    assert!(get_for_owner(&pool, "legacy-local", PROVIDER_GITHUB)
        .await
        .unwrap()
        .is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn github_legacy_oauth_token_requires_reconnect_and_can_be_replaced(pool: PgPool) {
    let secret = test_secret_box();
    upsert_connected(
        &pool,
        "alice",
        PROVIDER_GITHUB,
        &json!({ "login": "alice" }),
        "gho_legacy_raw_token",
        &secret,
    )
    .await
    .unwrap();

    let connectors = PostgresAgentConnectors::new(
        pool.clone(),
        std::sync::Arc::new(test_secret_box()),
        GitHubClient::production(),
    );
    let err = connectors
        .dispatch_connector_tool("alice", "github_list_repositories", &json!({}))
        .await
        .unwrap_err();
    assert_eq!(err, ConnectorError::ReconnectRequired);
    assert_eq!(
        get_for_owner(&pool, "alice", PROVIDER_GITHUB)
            .await
            .unwrap()
            .unwrap()
            .status,
        "reconnect_required"
    );
    assert_eq!(
        decrypt_connector_secret(&pool, "alice", PROVIDER_GITHUB, &secret)
            .await
            .unwrap()
            .as_deref(),
        Some("gho_legacy_raw_token")
    );
    assert!(matches!(
        load_github_credential(&pool, "alice", &secret)
            .await
            .unwrap(),
        GitHubCredentialLoad::ReconnectRequired | GitHubCredentialLoad::Legacy
    ));

    upsert_github_app_credential(
        &pool,
        "alice",
        &json!({ "githubUser": { "login": "alice" } }),
        &app_credential("ghu_new", "ghr_new", 3600),
        &secret,
    )
    .await
    .unwrap();
    assert!(matches!(
        load_github_credential(&pool, "alice", &secret)
            .await
            .unwrap(),
        GitHubCredentialLoad::App(_)
    ));
    assert_eq!(
        get_for_owner(&pool, "alice", PROVIDER_GITHUB)
            .await
            .unwrap()
            .unwrap()
            .status,
        "connected"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn github_fresh_token_does_not_refresh(pool: PgPool) {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/login/oauth/access_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(0)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/user/installations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "installations": [installation_json(11, "octocat", "User")]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/user/installations/11/repositories"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "repositories": [repo_json("octocat/Hello-World", false, "demo")]
        })))
        .mount(&mock)
        .await;

    let secret = test_secret_box();
    upsert_github_app_credential(
        &pool,
        "alice",
        &json!({}),
        &app_credential("ghu_fresh", "ghr_fresh", 8 * 3600),
        &secret,
    )
    .await
    .unwrap();
    let connectors =
        PostgresAgentConnectors::new(pool.clone(), secret.into(), github_client(&mock));
    connectors
        .dispatch_connector_tool("alice", "github_list_repositories", &json!({}))
        .await
        .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn github_near_expiry_refreshes_and_replaces_token_pair(pool: PgPool) {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/login/oauth/access_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "ghu_rotated",
            "expires_in": 28800,
            "refresh_token": "ghr_rotated",
            "refresh_token_expires_in": 15897600,
            "token_type": "bearer",
            "scope": ""
        })))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/user/installations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "installations": [installation_json(11, "octocat", "User")]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/user/installations/11/repositories"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "repositories": [repo_json("octocat/Hello-World", false, "demo")]
        })))
        .mount(&mock)
        .await;

    let secret = test_secret_box();
    upsert_github_app_credential(
        &pool,
        "alice",
        &json!({}),
        &app_credential("ghu_old", "ghr_old", 10),
        &secret,
    )
    .await
    .unwrap();
    let connectors = PostgresAgentConnectors::new(
        pool.clone(),
        std::sync::Arc::new(test_secret_box()),
        github_client(&mock),
    );
    connectors
        .dispatch_connector_tool("alice", "github_list_repositories", &json!({}))
        .await
        .unwrap();
    let plaintext = decrypt_connector_secret(&pool, "alice", PROVIDER_GITHUB, &secret)
        .await
        .unwrap()
        .unwrap();
    assert!(plaintext.contains("ghu_rotated"));
    assert!(plaintext.contains("ghr_rotated"));
    assert!(!plaintext.contains("ghu_old"));
    assert!(!plaintext.contains("ghr_old"));
}

#[sqlx::test(migrations = "./migrations")]
async fn github_failed_refresh_marks_reconnect_required_once(pool: PgPool) {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/login/oauth/access_token"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "error": "bad_refresh_token"
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let secret = test_secret_box();
    upsert_github_app_credential(
        &pool,
        "alice",
        &json!({}),
        &app_credential("ghu_old", "ghr_old", 10),
        &secret,
    )
    .await
    .unwrap();
    let connectors = PostgresAgentConnectors::new(
        pool.clone(),
        std::sync::Arc::new(test_secret_box()),
        github_client(&mock),
    );
    let err = connectors
        .dispatch_connector_tool("alice", "github_list_repositories", &json!({}))
        .await
        .unwrap_err();
    assert_eq!(err, ConnectorError::ReconnectRequired);
    let loggable = cloud_host::redact::redact_secrets(&err.message());
    assert!(!loggable.contains("ghu_"));
    assert!(!loggable.contains("ghr_"));
    assert_eq!(
        get_for_owner(&pool, "alice", PROVIDER_GITHUB)
            .await
            .unwrap()
            .unwrap()
            .status,
        "reconnect_required"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn github_tools_are_scoped_to_installation_repositories(pool: PgPool) {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user/installations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "installations": [
                installation_json(11, "octocat", "User"),
                installation_json(22, "acme", "Organization")
            ]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/user/installations/11/repositories"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "repositories": [repo_json("octocat/Hello-World", false, "public demo")]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/user/installations/22/repositories"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "repositories": [repo_json("acme/private", true, "private work")]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/search/repositories"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "items": [] })))
        .expect(0)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/user/repos"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(0)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/private"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "full_name": "acme/private",
            "private": true
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/outsider/public"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "full_name": "outsider/public"
        })))
        .expect(0)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/outsider/secret"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "full_name": "outsider/secret"
        })))
        .expect(0)
        .mount(&mock)
        .await;

    let secret = test_secret_box();
    upsert_github_app_credential(
        &pool,
        "alice",
        &json!({}),
        &app_credential("ghu_scope", "ghr_scope", 3600),
        &secret,
    )
    .await
    .unwrap();
    let connectors = PostgresAgentConnectors::new(
        pool.clone(),
        std::sync::Arc::new(test_secret_box()),
        github_client(&mock),
    );

    let listed = connectors
        .dispatch_connector_tool("alice", "github_list_repositories", &json!({}))
        .await
        .unwrap();
    let repos = listed["repositories"].as_array().unwrap();
    assert_eq!(repos.len(), 2);

    let search = connectors
        .dispatch_connector_tool(
            "alice",
            "github_search_repositories",
            &json!({ "query": "private work" }),
        )
        .await
        .unwrap();
    let items = search["result"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["full_name"], "acme/private");

    let allowed = connectors
        .dispatch_connector_tool(
            "alice",
            "github_get_repository",
            &json!({ "owner": "acme", "repo": "private" }),
        )
        .await
        .unwrap();
    assert!(allowed["ok"].as_bool().unwrap());

    for (owner, repo) in [("outsider", "public"), ("outsider", "secret")] {
        let err = connectors
            .dispatch_connector_tool(
                "alice",
                "github_get_repository",
                &json!({ "owner": owner, "repo": repo }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ConnectorError::Provider(_)));
        assert!(err.message().contains("authorized installation set"));
    }

    let bob = connectors
        .dispatch_connector_tool("bob", "github_list_repositories", &json!({}))
        .await
        .unwrap_err();
    assert_eq!(bob, ConnectorError::NotConnected);

    for name in [
        "github_list_repositories",
        "github_search_repositories",
        "github_get_repository",
        "github_get_file_contents",
        "github_list_issues",
        "github_get_issue",
        "github_list_pull_requests",
        "github_get_pull_request",
    ] {
        assert!(agent_core::is_github_connector_tool(name), "{name}");
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn github_provider_errors_redact_app_tokens(pool: PgPool) {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user/installations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "installations": [installation_json(11, "octocat", "User")]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/user/installations/11/repositories"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "repositories": [repo_json("octocat/Hello-World", false, "demo")]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/Hello-World"))
        .respond_with(
            ResponseTemplate::new(403)
                .set_body_string("forbidden ghu_leaked_access ghr_leaked_refresh"),
        )
        .mount(&mock)
        .await;

    let secret = test_secret_box();
    upsert_github_app_credential(
        &pool,
        "alice",
        &json!({}),
        &app_credential("ghu_scope", "ghr_scope", 3600),
        &secret,
    )
    .await
    .unwrap();
    let connectors =
        PostgresAgentConnectors::new(pool.clone(), secret.into(), github_client(&mock));
    let err = connectors
        .dispatch_connector_tool(
            "alice",
            "github_get_repository",
            &json!({ "owner": "octocat", "repo": "Hello-World" }),
        )
        .await
        .unwrap_err();
    let message = err.message();
    assert!(!message.contains("ghu_leaked_access"));
    assert!(!message.contains("ghr_leaked_refresh"));
}

#[sqlx::test(migrations = "./migrations")]
async fn github_oauth_start_rejects_invalid_return_to(pool: PgPool) {
    let mock = MockServer::start().await;
    let mut state = AppState::new(pool.clone(), github_http_config());
    state.github_client = github_client(&mock);
    let app = build_router(state);

    for return_to in [
        "/app/connectors",
        "https://evil.example",
        "//evil",
        "/app/work/x",
    ] {
        let (status, body) = json_auth(
            app.clone(),
            "POST",
            "/v1/connectors/github/oauth/start",
            json!({ "returnTo": return_to }),
        )
        .await;
        assert_eq!(status, http::StatusCode::BAD_REQUEST, "{return_to}");
        assert!(body["error"].as_str().unwrap_or("").contains("returnTo"));
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn github_oauth_return_to_round_trips_through_complete(pool: PgPool) {
    let mock = MockServer::start().await;
    mock_token_exchange(&mock, "ghu_access_live", "ghr_refresh_live").await;
    mock_user_and_installs(
        &mock,
        &[(11, "octocat", "User")],
        &[(11, vec![repo_json("octocat/Hello-World", false, "demo")])],
    )
    .await;

    let mut state = AppState::new(pool.clone(), github_http_config());
    state.github_client = github_client(&mock);
    let app = build_router(state);

    let return_to = "/app/bots/bot_1?conversation=c1";
    let (status, start) = json_auth(
        app.clone(),
        "POST",
        "/v1/connectors/github/oauth/start",
        json!({ "returnTo": return_to }),
    )
    .await;
    assert_eq!(status, http::StatusCode::OK);
    let oauth_state = start["state"].as_str().unwrap();

    let (status, connected) = json_auth(
        app.clone(),
        "POST",
        "/v1/connectors/github/oauth/complete",
        json!({
            "code": "install-code",
            "state": oauth_state
        }),
    )
    .await;
    assert_eq!(status, http::StatusCode::OK);
    assert_eq!(connected["status"], "connected");
    assert_eq!(connected["returnTo"], return_to);
}

#[sqlx::test(migrations = "./migrations")]
async fn github_oauth_complete_notifies_pending_github_waiters(pool: PgPool) {
    let mock = MockServer::start().await;
    mock_token_exchange(&mock, "ghu_access_live", "ghr_refresh_live").await;
    mock_user_and_installs(
        &mock,
        &[(11, "octocat", "User")],
        &[(11, vec![repo_json("octocat/Hello-World", false, "demo")])],
    )
    .await;

    let mut state = AppState::new(pool.clone(), github_http_config());
    state.github_client = github_client(&mock);
    let need_id = "cneed_notify_1";
    sqlx::query(
        r#"
        INSERT INTO connector_need_requests (
            id, owner_id, run_id, bot_id, provider, tool_name, reason_kind, status, created_at
        )
        VALUES ($1, 'legacy-local', 'run_notify', 'bot_1', 'github', 'github_list_repositories', 'disconnected', 'pending', NOW())
        "#,
    )
    .bind(need_id)
    .execute(&pool)
    .await
    .unwrap();
    let handle = state.connector_needs.registry.subscribe(need_id);
    let app = build_router(state);

    let (status, start) = json_auth(
        app.clone(),
        "POST",
        "/v1/connectors/github/oauth/start",
        json!({}),
    )
    .await;
    assert_eq!(status, http::StatusCode::OK);
    let oauth_state = start["state"].as_str().unwrap();

    let notified = tokio::spawn(async move {
        handle.notified().await;
    });
    let (status, connected) = json_auth(
        app,
        "POST",
        "/v1/connectors/github/oauth/complete",
        json!({
            "code": "install-code",
            "state": oauth_state
        }),
    )
    .await;
    assert_eq!(status, http::StatusCode::OK);
    assert_eq!(connected["status"], "connected");
    tokio::time::timeout(std::time::Duration::from_secs(2), notified)
        .await
        .expect("waiter notified")
        .expect("notify task");
}

#[sqlx::test(migrations = "./migrations")]
async fn github_status_reports_not_connectable_without_github_app(pool: PgPool) {
    let state = AppState::new(pool, test_config());
    let app = build_router(state);
    let (status, body) = json_auth(app, "GET", "/v1/connectors/github", json!({})).await;
    assert_eq!(status, http::StatusCode::OK);
    assert_eq!(body["status"], "disconnected");
    assert_eq!(body["connectable"], false);
}

#[sqlx::test(migrations = "./migrations")]
async fn github_oauth_start_rejects_unconfigured_host(pool: PgPool) {
    let state = AppState::new(pool, test_config());
    let app = build_router(state);
    let (status, body) =
        json_auth(app, "POST", "/v1/connectors/github/oauth/start", json!({})).await;
    assert_eq!(status, http::StatusCode::BAD_REQUEST);
    assert!(body["error"]
        .as_str()
        .unwrap_or("")
        .contains("GitHub isn't available on this host"));
}
