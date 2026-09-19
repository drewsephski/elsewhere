use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use agent_core::RunStore;
use base64::Engine;
use chrono::{Duration as ChronoDuration, Utc};
use cloud_host::connector_need::{ConnectorNeedService, ConnectorNeedWaitRegistry};
use cloud_host::connectors::{
    classify_github_access,
    db::{
        mark_reconnect_required, store_oauth_state, upsert_github_app_credential, PROVIDER_GITHUB,
    },
    github_client::GitHubCredential,
    parse_bot_chat_return_to,
    secret::ConnectorSecretBox,
    service::PostgresAgentConnectors,
    ConnectorNeedReason, ConnectorNeedRequest, GitHubClient, GithubAccess,
};
use cloud_host::db::postgres_run_store::PostgresRunStore;
use cloud_host::db::resources::{insert_bot, insert_computer_placeholder};
use cloud_host::events::cloud_event_sink::CloudEventSink;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[path = "support/mod.rs"]
mod support;
use support::test_config;

fn test_secret_box() -> ConnectorSecretBox {
    let key = base64::engine::general_purpose::STANDARD.encode([7u8; 32]);
    ConnectorSecretBox::from_base64_key(&key).expect("test key")
}

fn github_client(mock: &MockServer) -> GitHubClient {
    GitHubClient::with_api_base(mock.uri(), mock.uri()).with_oauth(
        "Iv23lihP3j9Z6kZU0s9p".into(),
        "github-app-client-secret".into(),
    )
}

fn app_credential(access: &str, refresh: &str) -> GitHubCredential {
    GitHubCredential {
        kind: "github_app_user".into(),
        access_token: access.into(),
        refresh_token: refresh.into(),
        access_expires_at: Utc::now() + ChronoDuration::hours(8),
        refresh_expires_at: Utc::now() + ChronoDuration::days(180),
        token_type: "bearer".into(),
    }
}

fn installation_json(id: i64, login: &str, account_type: &str) -> serde_json::Value {
    json!({
        "id": id,
        "account": { "login": login, "id": id, "type": account_type },
        "repository_selection": "selected",
        "permissions": { "contents": "write" }
    })
}

fn repo_json(full_name: &str) -> serde_json::Value {
    let name = full_name.split('/').nth(1).unwrap_or(full_name);
    let owner = full_name.split('/').next().unwrap_or(full_name);
    json!({
        "full_name": full_name,
        "name": name,
        "private": false,
        "description": "",
        "owner": { "login": owner }
    })
}

async fn mock_installs(mock: &MockServer, repos: Vec<serde_json::Value>) {
    Mock::given(method("GET"))
        .and(path("/user/installations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total_count": 1,
            "installations": [installation_json(11, "acme", "Organization")]
        })))
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/user/installations/11/repositories"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total_count": repos.len(),
            "repositories": repos
        })))
        .mount(mock)
        .await;
}

async fn seed_run(pool: &PgPool, owner_id: &str, run_id: &str, request_id: &str) -> String {
    let computer = insert_computer_placeholder(pool, owner_id, "c")
        .await
        .unwrap();
    let bot = insert_bot(
        pool,
        owner_id,
        "b",
        "i",
        "gpt-5.6-luna",
        Some(computer.id.as_str()),
        "responses",
        "sky-wisp",
    )
    .await
    .unwrap();
    let conv_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO conversations (id, owner_id, bot_id, created_at, updated_at) VALUES ($1, $2, $3, NOW(), NOW())",
    )
    .bind(&conv_id)
    .bind(owner_id)
    .bind(&bot.id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO agent_runs (id, owner_id, request_id, bot_id, conversation_id, computer_id, model, status, step_count, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, 'gpt-5.6-luna', 'running', 0, NOW(), NOW())
        "#,
    )
    .bind(run_id)
    .bind(owner_id)
    .bind(request_id)
    .bind(&bot.id)
    .bind(&conv_id)
    .bind(&computer.id)
    .execute(pool)
    .await
    .unwrap();
    bot.id
}

#[test]
fn parse_bot_chat_return_to_rejects_non_bot_paths() {
    assert!(parse_bot_chat_return_to("/app/connectors").is_none());
    assert!(parse_bot_chat_return_to("https://evil.example").is_none());
    assert!(parse_bot_chat_return_to("//evil").is_none());
    assert!(parse_bot_chat_return_to("/app/work/x").is_none());
    assert_eq!(
        parse_bot_chat_return_to("/app/bots/bot_1?conversation=c1").as_deref(),
        Some("/app/bots/bot_1?conversation=c1")
    );
}

#[test]
fn github_oauth_ready_requires_secret_and_app_fields() {
    let mut config = test_config();
    let secret = test_secret_box();
    assert!(!config.github_oauth_ready(None));
    assert!(!config.github_oauth_ready(Some(&secret)));
    config.github_app_slug = Some("elsewhere".into());
    config.github_client_id = Some("Iv23client".into());
    config.github_client_secret = Some("client-secret".into());
    config.github_oauth_redirect_uri = Some("https://example.invalid/callback".into());
    assert!(!config.github_oauth_ready(None));
    assert!(config.github_oauth_ready(Some(&secret)));
}

#[tokio::test]
async fn classify_host_unconfigured_when_oauth_is_not_ready() {
    let access = classify_github_access(
        None,
        false,
        "alice",
        "github_get_repository",
        &json!({ "owner": "acme", "repo": "elsewhere" }),
    )
    .await
    .unwrap();
    assert_eq!(
        access,
        GithubAccess::Need(ConnectorNeedReason::HostUnconfigured)
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn classify_disconnected_when_github_is_missing(pool: PgPool) {
    let mock = MockServer::start().await;
    let connectors =
        PostgresAgentConnectors::new(pool, test_secret_box().into(), github_client(&mock));
    let access = classify_github_access(
        Some(connectors.as_ref()),
        true,
        "alice",
        "github_list_repositories",
        &json!({}),
    )
    .await
    .unwrap();
    assert_eq!(
        access,
        GithubAccess::Need(ConnectorNeedReason::Disconnected)
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn classify_reconnect_required(pool: PgPool) {
    let mock = MockServer::start().await;
    let secret = test_secret_box();
    upsert_github_app_credential(
        &pool,
        "alice",
        &json!({}),
        &app_credential("ghu_a", "ghr_a"),
        &secret,
    )
    .await
    .unwrap();
    mark_reconnect_required(&pool, "alice", PROVIDER_GITHUB)
        .await
        .unwrap();
    let connectors = PostgresAgentConnectors::new(pool, secret.into(), github_client(&mock));
    let access = classify_github_access(
        Some(connectors.as_ref()),
        true,
        "alice",
        "github_list_repositories",
        &json!({}),
    )
    .await
    .unwrap();
    assert_eq!(
        access,
        GithubAccess::Need(ConnectorNeedReason::ReconnectRequired)
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn classify_empty_authorization_for_list_tools(pool: PgPool) {
    let mock = MockServer::start().await;
    mock_installs(&mock, vec![]).await;
    let secret = test_secret_box();
    upsert_github_app_credential(
        &pool,
        "alice",
        &json!({}),
        &app_credential("ghu_a", "ghr_a"),
        &secret,
    )
    .await
    .unwrap();
    let connectors = PostgresAgentConnectors::new(pool, secret.into(), github_client(&mock));
    let access = classify_github_access(
        Some(connectors.as_ref()),
        true,
        "alice",
        "github_list_repositories",
        &json!({}),
    )
    .await
    .unwrap();
    assert_eq!(
        access,
        GithubAccess::Need(ConnectorNeedReason::EmptyAuthorization)
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn classify_unauthorized_repo_when_catalog_omits_target(pool: PgPool) {
    let mock = MockServer::start().await;
    mock_installs(&mock, vec![repo_json("acme/other")]).await;
    let secret = test_secret_box();
    upsert_github_app_credential(
        &pool,
        "alice",
        &json!({}),
        &app_credential("ghu_a", "ghr_a"),
        &secret,
    )
    .await
    .unwrap();
    let connectors = PostgresAgentConnectors::new(pool, secret.into(), github_client(&mock));
    let access = classify_github_access(
        Some(connectors.as_ref()),
        true,
        "alice",
        "github_get_file_contents",
        &json!({ "owner": "acme", "repo": "elsewhere" }),
    )
    .await
    .unwrap();
    assert_eq!(
        access,
        GithubAccess::Need(ConnectorNeedReason::UnauthorizedRepo {
            owner: "acme".into(),
            repo: "elsewhere".into(),
        })
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn second_wait_joins_pending_need_for_run_and_provider(pool: PgPool) {
    let owner = "alice";
    let run_id = format!("run_{}", Uuid::new_v4());
    let request_id = format!("req_{}", Uuid::new_v4());
    let bot_id = seed_run(&pool, owner, &run_id, &request_id).await;
    let mock = MockServer::start().await;
    let connectors =
        PostgresAgentConnectors::new(pool.clone(), test_secret_box().into(), github_client(&mock));
    let needs = ConnectorNeedService {
        pool: pool.clone(),
        registry: Arc::new(ConnectorNeedWaitRegistry::default()),
        timeout: Duration::from_secs(8),
    };
    let (sink, _rx) = CloudEventSink::new();
    let events = Arc::new(sink);
    let store: Arc<dyn RunStore> = Arc::new(PostgresRunStore::new(pool.clone()));
    let cancel = Arc::new(AtomicBool::new(false));
    let request = ConnectorNeedRequest {
        owner_id: owner.into(),
        run_id: run_id.clone(),
        bot_id,
        tool_name: "github_list_repositories".into(),
        reason: ConnectorNeedReason::Disconnected,
        arguments: json!({}),
    };

    let first = {
        let needs = needs.clone();
        let request = request.clone();
        let cancel = cancel.clone();
        let events = events.clone();
        let store = store.clone();
        let connectors = connectors.clone();
        tokio::spawn(async move {
            needs
                .request_and_wait(request, &cancel, &events, &store, connectors.as_ref())
                .await
        })
    };
    let second = {
        let needs = needs.clone();
        let request = request.clone();
        let cancel = cancel.clone();
        let events = events.clone();
        let store = store.clone();
        let connectors = connectors.clone();
        tokio::spawn(async move {
            needs
                .request_and_wait(request, &cancel, &events, &store, connectors.as_ref())
                .await
        })
    };

    let pending: (i64,) = loop {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*)::bigint FROM connector_need_requests WHERE run_id = $1 AND status = 'pending'",
        )
        .bind(&run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        if count.0 == 1 {
            break count;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    assert_eq!(pending.0, 1);
    let events: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM run_events WHERE request_id = $1 AND event_type = 'connector_needed'",
    )
    .bind(&request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(events.0, 1);

    cancel.store(true, Ordering::Relaxed);
    let _ = first.await;
    let _ = second.await;
}

#[sqlx::test(migrations = "./migrations")]
async fn oauth_that_omits_repo_leaves_unauthorized_need_pending(pool: PgPool) {
    let owner = "alice";
    let run_id = format!("run_{}", Uuid::new_v4());
    let request_id = format!("req_{}", Uuid::new_v4());
    let bot_id = seed_run(&pool, owner, &run_id, &request_id).await;
    let mock = MockServer::start().await;
    mock_installs(&mock, vec![repo_json("acme/other")]).await;
    let secret = Arc::new(test_secret_box());
    let connectors =
        PostgresAgentConnectors::new(pool.clone(), secret.clone(), github_client(&mock));
    let needs = ConnectorNeedService {
        pool: pool.clone(),
        registry: Arc::new(ConnectorNeedWaitRegistry::default()),
        timeout: Duration::from_secs(8),
    };
    let (sink, _rx) = CloudEventSink::new();
    let events = Arc::new(sink);
    let store: Arc<dyn RunStore> = Arc::new(PostgresRunStore::new(pool.clone()));
    let cancel = Arc::new(AtomicBool::new(false));
    let request = ConnectorNeedRequest {
        owner_id: owner.into(),
        run_id: run_id.clone(),
        bot_id,
        tool_name: "github_get_file_contents".into(),
        reason: ConnectorNeedReason::Disconnected,
        arguments: json!({ "owner": "acme", "repo": "elsewhere" }),
    };

    let wait = {
        let needs = needs.clone();
        let cancel = cancel.clone();
        let events = events.clone();
        let store = store.clone();
        let connectors = connectors.clone();
        tokio::spawn(async move {
            needs
                .request_and_wait(request, &cancel, &events, &store, connectors.as_ref())
                .await
        })
    };

    let need_id: String = loop {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM connector_need_requests WHERE run_id = $1 AND status = 'pending'",
        )
        .bind(&run_id)
        .fetch_optional(&pool)
        .await
        .unwrap();
        if let Some((id,)) = row {
            break id;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    };

    upsert_github_app_credential(
        &pool,
        owner,
        &json!({}),
        &app_credential("ghu_a", "ghr_a"),
        secret.as_ref(),
    )
    .await
    .unwrap();
    connectors.invalidate_github_catalog(owner);
    needs.notify_owner_github_changed(owner).await.unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;

    let status: (String,) =
        sqlx::query_as("SELECT status FROM connector_need_requests WHERE id = $1")
            .bind(&need_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status.0, "pending");

    cancel.store(true, Ordering::Relaxed);
    let _ = wait.await;
}

#[sqlx::test(migrations = "./migrations")]
async fn store_oauth_state_persists_return_to(pool: PgPool) {
    let expires_at = Utc::now() + ChronoDuration::minutes(10);
    store_oauth_state(
        &pool,
        "state-return",
        "alice",
        PROVIDER_GITHUB,
        expires_at,
        Some("/app/bots/bot_1?conversation=c1"),
    )
    .await
    .unwrap();
    let stored: Option<(Option<String>,)> =
        sqlx::query_as("SELECT return_to FROM connector_oauth_states WHERE state = $1")
            .bind("state-return")
            .fetch_optional(&pool)
            .await
            .unwrap();
    assert_eq!(
        stored.unwrap().0.as_deref(),
        Some("/app/bots/bot_1?conversation=c1")
    );
}
