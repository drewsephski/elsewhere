use base64::Engine;
use cloud_host::channels::db::{load_access_token, upsert_slack_connection};
use cloud_host::channels::delivery::{self, split_outbound_body};
use cloud_host::channels::slack::signature::sign_slack_request;
use cloud_host::channels::PROVIDER_SLACK;
use cloud_host::connectors::secret::ConnectorSecretBox;
use cloud_host::conversation;
use cloud_host::db::resources;
use cloud_host::redact::redact_secrets;
use cloud_host::work;
use cloud_host::{build_router, AppState, Config};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SIGNING_SECRET: &str = "signing-secret";
const OWNER: &str = "legacy-local";
const MOCK_SLACK_BOT_TOKEN: &str = "xoxb-bot-token-secret";

/// OAuth complete must return only `ChannelConnectionSummary` fields — never tokens or Slack ids.
fn assert_slack_oauth_complete_response_excludes_secrets(conn: &Value, response_body: &str) {
    assert!(
        !response_body.contains(MOCK_SLACK_BOT_TOKEN),
        "response must not contain the plaintext Slack bot token"
    );
    let obj = conn
        .as_object()
        .expect("oauth complete response must be a JSON object");
    const FORBIDDEN_KEYS: &[&str] = &[
        "accessToken",
        "access_token",
        "botToken",
        "token",
        "installerExternalUserId",
        "externalWorkspaceId",
        "botUserId",
        "workspaceId",
        "ciphertext",
        "nonce",
    ];
    for key in FORBIDDEN_KEYS {
        assert!(
            obj.get(*key).is_none(),
            "response must not include secret field `{}`",
            key
        );
    }
    const ALLOWED_KEYS: &[&str] = &[
        "id",
        "provider",
        "status",
        "enabled",
        "workspaceName",
        "defaultBotId",
        "defaultBotName",
        "connectedAt",
        "updatedAt",
    ];
    for key in obj.keys() {
        assert!(
            ALLOWED_KEYS.contains(&key.as_str()),
            "unexpected field `{}` in oauth complete response",
            key
        );
    }
}

fn secret_key() -> String {
    base64::engine::general_purpose::STANDARD.encode([9u8; 32])
}

fn secret_box() -> ConnectorSecretBox {
    ConnectorSecretBox::from_base64_key(&secret_key()).expect("test key")
}

fn test_config(slack_api_base: Option<String>) -> Config {
    Config {
        database_url: std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://elsewhere:elsewhere@127.0.0.1:5432/elsewhere".into()),
        openai_api_key: Some("test-key".into()),
        sprite_token: "test-sprite".into(),
        api_token: "test-token".into(),
        auth_mode: cloud_host::config::AuthMode::InternalToken,
        jwt_issuer: None,
        jwt_audience: None,
        jwt_jwks_url: None,
        cors_web_origin: Some("http://localhost:3000".into()),
        allow_codex_login: false,
        sprites_api_base: "http://127.0.0.1:9".into(),
        max_concurrent_runs: 2,
        run_timeout_secs: 120,
        bind_addr: "127.0.0.1:0".into(),
        run_engine: cloud_host::run_engine_select::RunEngineMode::Responses,
        codex_executable: None,
        codex_profiles_dir: None,
        browser_profiles_dir: None,
        tool_approval_timeout_secs: 300,
        enforce_tool_approvals_internal: false,
        legacy_local_approval_bypass: false,
        browser_enabled: false,
        connector_secret_key: Some(secret_key()),
        github_client_id: None,
        github_client_secret: None,
        github_oauth_redirect_uri: None,
        slack_client_id: Some("slack-client".into()),
        slack_client_secret: Some("slack-secret".into()),
        slack_signing_secret: Some(SIGNING_SECRET.into()),
        slack_oauth_redirect_uri: Some("http://localhost:3000/app/channels/slack/callback".into()),
        slack_api_base: slack_api_base.unwrap_or_else(|| "https://slack.com/api".into()),
        local_mac_credential_key: None,
    }
}

async fn insert_bot(pool: &PgPool, owner: &str) -> resources::BotRow {
    let computer = resources::insert_computer_placeholder(pool, owner, "Research computer")
        .await
        .unwrap();
    resources::insert_bot(
        pool,
        owner,
        "Scout",
        "Keep sources with your findings",
        "gpt-5.6-luna",
        Some(&computer.id),
        "codex",
        "sky-wisp",
    )
    .await
    .unwrap()
}

async fn connect_slack(pool: &PgPool, owner: &str, bot_id: &str, workspace: &str, installer: &str) {
    upsert_slack_connection(
        pool,
        owner,
        workspace,
        Some("Acme"),
        installer,
        Some("UBOT"),
        bot_id,
        "xoxb-test-token",
        &secret_box(),
    )
    .await
    .unwrap();
}

fn signed_request(body: &str, ts: i64) -> (String, String) {
    let ts = ts.to_string();
    let sig = sign_slack_request(SIGNING_SECRET, &ts, body.as_bytes());
    (ts, sig)
}

async fn post_slack_event(
    app: axum::Router,
    body: &str,
    ts: i64,
    sig: &str,
) -> (http::StatusCode, Value) {
    let response = app
        .oneshot(
            http::Request::builder()
                .method("POST")
                .uri("/internal/hooks/channels/slack/events")
                .header("content-type", "application/json")
                .header("x-slack-request-timestamp", ts.to_string())
                .header("x-slack-signature", sig)
                .body(axum::body::Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(json!({}));
    (status, json)
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

#[sqlx::test(migrations = "./migrations")]
async fn slack_signature_security(pool: PgPool) {
    let state = AppState::new(pool, test_config(None));
    let app = build_router(state);
    let body = r#"{"type":"url_verification","challenge":"abc"}"#;
    let now = chrono::Utc::now().timestamp();
    let (_, sig) = signed_request(body, now);

    let (status, json) = post_slack_event(app.clone(), body, now, &sig).await;
    assert_eq!(status, http::StatusCode::OK);
    assert_eq!(json["challenge"], "abc");

    let (status, _) = post_slack_event(app.clone(), body, now, "v0=deadbeef").await;
    assert_eq!(status, http::StatusCode::UNAUTHORIZED);

    let (status, _) = post_slack_event(app.clone(), body, now - 400, &sig).await;
    assert_eq!(status, http::StatusCode::UNAUTHORIZED);

    let raw = r#"{"type":"url_verification","challenge":"raw-bytes"}"#;
    let (_, sig) = signed_request(raw, now);
    let reparsed =
        serde_json::to_string(&json!({"type":"url_verification","challenge":"raw-bytes"})).unwrap();
    if reparsed != raw {
        let (status, _) = post_slack_event(app.clone(), &reparsed, now, &sig).await;
        assert_eq!(status, http::StatusCode::UNAUTHORIZED);
    }
    let (status, json) = post_slack_event(app, raw, now, &sig).await;
    assert_eq!(status, http::StatusCode::OK);
    assert_eq!(json["challenge"], "raw-bytes");
}

#[sqlx::test(migrations = "./migrations")]
async fn slack_oauth_state_and_encrypted_token(pool: PgPool) {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth.v2.access"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ok": true,
            "access_token": "xoxb-bot-token-secret",
            "bot_user_id": "UBOT",
            "team": {"id": "T1", "name": "Acme"},
            "authed_user": {"id": "UINSTALL"}
        })))
        .mount(&mock)
        .await;

    let bot = insert_bot(&pool, OWNER).await;
    let state = AppState::new(pool.clone(), test_config(Some(mock.uri())));
    let app = build_router(state.clone());

    let (status, start) = json_auth(
        app.clone(),
        "POST",
        "/v1/channels/slack/oauth/start",
        json!({"botId": bot.id}),
    )
    .await;
    assert_eq!(status, http::StatusCode::OK);
    let authorize_url = start["authorizeUrl"].as_str().unwrap();
    assert!(authorize_url.contains("chat%3Awrite"));
    assert!(authorize_url.contains("app_mentions%3Aread"));
    assert!(authorize_url.contains("im%3Ahistory"));
    let state_token = authorize_url
        .split("state=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap();

    let expired = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO channel_oauth_states (state, owner_id, provider, bot_id, expires_at) VALUES ($1,$2,'slack',$3, NOW() - INTERVAL '1 minute')",
    )
    .bind(&expired)
    .bind(OWNER)
    .bind(&bot.id)
    .execute(&pool)
    .await
    .unwrap();
    let (status, _) = json_auth(
        app.clone(),
        "POST",
        "/v1/channels/slack/oauth/complete",
        json!({"code": "x", "state": expired}),
    )
    .await;
    assert_eq!(status, http::StatusCode::BAD_REQUEST);

    let (status, conn) = json_auth(
        app.clone(),
        "POST",
        "/v1/channels/slack/oauth/complete",
        json!({"code": "real", "state": state_token}),
    )
    .await;
    assert_eq!(status, http::StatusCode::OK);
    assert_eq!(conn["workspaceName"], "Acme");
    let response_body = serde_json::to_string(&conn).unwrap();
    assert_slack_oauth_complete_response_excludes_secrets(&conn, &response_body);

    let (status, _) = json_auth(
        app.clone(),
        "POST",
        "/v1/channels/slack/oauth/complete",
        json!({"code": "real", "state": state_token}),
    )
    .await;
    assert_eq!(status, http::StatusCode::BAD_REQUEST);

    let connection_id = conn["id"].as_str().unwrap();
    let stored_ciphertext: Vec<u8> = sqlx::query_scalar(
        "SELECT ciphertext FROM channel_connection_secrets WHERE connection_id = $1",
    )
    .bind(connection_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let stored_as_utf8 = String::from_utf8_lossy(&stored_ciphertext);
    assert_ne!(
        stored_ciphertext.as_slice(),
        MOCK_SLACK_BOT_TOKEN.as_bytes(),
        "access token must be encrypted at rest, not stored as plaintext"
    );
    assert!(
        !stored_as_utf8.contains(MOCK_SLACK_BOT_TOKEN),
        "ciphertext must not embed the plaintext bot token"
    );

    let token = load_access_token(&pool, connection_id, &secret_box())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(token, MOCK_SLACK_BOT_TOKEN);

    Mock::given(method("POST"))
        .and(path("/apps.uninstall"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&mock)
        .await;
    let (status, disconnected) = json_auth(
        app,
        "DELETE",
        &format!("/v1/channels/{connection_id}"),
        json!({}),
    )
    .await;
    assert_eq!(status, http::StatusCode::OK);
    assert_eq!(disconnected["status"], "disconnected");
    assert!(load_access_token(&pool, connection_id, &secret_box())
        .await
        .unwrap()
        .is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn foreign_connection_is_inaccessible(pool: PgPool) {
    let bot = insert_bot(&pool, "alice").await;
    let row = upsert_slack_connection(
        &pool,
        "alice",
        "T9",
        Some("Hidden"),
        "U9",
        None,
        &bot.id,
        "xoxb-alice",
        &secret_box(),
    )
    .await
    .unwrap();
    let state = AppState::new(pool, test_config(None));
    let app = build_router(state);
    let (status, body) = json_auth(app.clone(), "GET", "/v1/channels", json!({})).await;
    assert_eq!(status, http::StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 0);
    let (status, _) = json_auth(
        app,
        "DELETE",
        &format!("/v1/channels/{}", row.id),
        json!({}),
    )
    .await;
    assert_eq!(status, http::StatusCode::NOT_FOUND);
}

fn dm_event(event_id: &str, user: &str, channel: &str, text: &str) -> String {
    serde_json::to_string(&json!({
        "token": "ignored",
        "team_id": "T1",
        "api_app_id": "A1",
        "event": {
            "type": "message",
            "channel": channel,
            "user": user,
            "text": text,
            "ts": "1710000000.000001",
            "channel_type": "im"
        },
        "type": "event_callback",
        "event_id": event_id,
        "event_time": 1710000000
    }))
    .unwrap()
}

fn mention_event(
    event_id: &str,
    user: &str,
    channel: &str,
    ts: &str,
    thread_ts: Option<&str>,
    text: &str,
) -> String {
    let mut event = json!({
        "type": "app_mention",
        "user": user,
        "text": text,
        "ts": ts,
        "channel": channel
    });
    if let Some(thread_ts) = thread_ts {
        event["thread_ts"] = json!(thread_ts);
    }
    serde_json::to_string(&json!({
        "team_id": "T1",
        "event": event,
        "type": "event_callback",
        "event_id": event_id
    }))
    .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn installer_events_admitted_and_others_ignored(pool: PgPool) {
    let bot = insert_bot(&pool, OWNER).await;
    connect_slack(&pool, OWNER, &bot.id, "T1", "UINSTALL").await;
    let state = AppState::new(pool.clone(), test_config(None));
    let app = build_router(state);
    let now = chrono::Utc::now().timestamp();

    let body = dm_event("EvDM1", "UINSTALL", "D1", "hello from dm");
    let (_, sig) = signed_request(&body, now);
    let (status, _) = post_slack_event(app.clone(), &body, now, &sig).await;
    assert_eq!(status, http::StatusCode::OK);

    let body = mention_event(
        "EvMN1",
        "UINSTALL",
        "C1",
        "1.0",
        None,
        "<@UBOT> hello channel",
    );
    let (_, sig) = signed_request(&body, now);
    let (status, _) = post_slack_event(app.clone(), &body, now, &sig).await;
    assert_eq!(status, http::StatusCode::OK);

    let body = dm_event("EvOTHER", "UCOWORKER", "D1", "coworker");
    let (_, sig) = signed_request(&body, now);
    let (status, _) = post_slack_event(app.clone(), &body, now, &sig).await;
    assert_eq!(status, http::StatusCode::OK);

    let mut bot_msg =
        serde_json::from_str::<Value>(&dm_event("EvBOT", "UBOT", "D1", "self")).unwrap();
    bot_msg["event"]["bot_id"] = json!("B1");
    let body = serde_json::to_string(&bot_msg).unwrap();
    let (_, sig) = signed_request(&body, now);
    let (status, _) = post_slack_event(app.clone(), &body, now, &sig).await;
    assert_eq!(status, http::StatusCode::OK);

    let mut edited =
        serde_json::from_str::<Value>(&dm_event("EvEDIT", "UINSTALL", "D1", "edit")).unwrap();
    edited["event"]["subtype"] = json!("message_changed");
    let body = serde_json::to_string(&edited).unwrap();
    let (_, sig) = signed_request(&body, now);
    let (status, _) = post_slack_event(app.clone(), &body, now, &sig).await;
    assert_eq!(status, http::StatusCode::OK);

    let huge = "x".repeat(9000);
    let body = dm_event("EvHUGE", "UINSTALL", "D1", &huge);
    let (_, sig) = signed_request(&body, now);
    let (status, _) = post_slack_event(app, &body, now, &sig).await;
    assert_eq!(status, http::StatusCode::OK);

    let runs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs WHERE owner_id = $1")
        .bind(OWNER)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(runs, 2);

    let ignored: Vec<String> = sqlx::query_scalar(
        "SELECT ignore_reason FROM channel_events WHERE status IN ('ignored','rejected') ORDER BY created_at",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(ignored.contains(&"non_installer".into()));
    assert!(ignored.contains(&"bot_message".into()) || ignored.contains(&"self_message".into()));
    assert!(ignored.contains(&"message_subtype".into()));
    assert!(ignored.contains(&"oversized".into()));
}

#[sqlx::test(migrations = "./migrations")]
async fn duplicate_event_id_creates_one_run(pool: PgPool) {
    let bot = insert_bot(&pool, OWNER).await;
    connect_slack(&pool, OWNER, &bot.id, "T1", "UINSTALL").await;
    let state = AppState::new(pool.clone(), test_config(None));
    let app = build_router(state);
    let now = chrono::Utc::now().timestamp();
    let body = dm_event("EvDUP", "UINSTALL", "D9", "once");
    let (_, sig) = signed_request(&body, now);
    let (a, _) = post_slack_event(app.clone(), &body, now, &sig).await;
    let (b, _) = post_slack_event(app, &body, now, &sig).await;
    assert_eq!(a, http::StatusCode::OK);
    assert_eq!(b, http::StatusCode::OK);
    let runs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs WHERE owner_id = $1")
        .bind(OWNER)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(runs, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn slack_conversations_never_become_primary_web_chat(pool: PgPool) {
    let bot = insert_bot(&pool, OWNER).await;
    connect_slack(&pool, OWNER, &bot.id, "T1", "UINSTALL").await;
    let mut tx = pool.begin().await.unwrap();
    let web = get_web_primary(&mut tx, OWNER, &bot.id).await;
    tx.commit().await.unwrap();

    let state = AppState::new(pool.clone(), test_config(None));
    let app = build_router(state.clone());
    let now = chrono::Utc::now().timestamp();
    let body = dm_event("EvWEB1", "UINSTALL", "DWEB", "slack first");
    let (_, sig) = signed_request(&body, now);
    post_slack_event(app.clone(), &body, now, &sig).await;

    let mut tx = pool.begin().await.unwrap();
    let web_after = get_web_primary(&mut tx, OWNER, &bot.id).await;
    tx.commit().await.unwrap();
    assert_eq!(web, web_after);

    let listed = app
        .oneshot(
            http::Request::builder()
                .uri(format!("/v1/conversations?limit=1&bot_id={}", bot.id))
                .header("authorization", "Bearer test-token")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = listed.into_body().collect().await.unwrap().to_bytes();
    let rows: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(rows[0]["id"], web);
}

async fn get_web_primary(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &str,
    bot_id: &str,
) -> String {
    conversation::get_or_create_primary_conversation_id_in_tx(tx, owner, bot_id)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn slack_thread_mapping_and_dm_stability(pool: PgPool) {
    let bot = insert_bot(&pool, OWNER).await;
    connect_slack(&pool, OWNER, &bot.id, "T1", "UINSTALL").await;
    let state = AppState::new(pool.clone(), test_config(None));
    let app = build_router(state);
    let now = chrono::Utc::now().timestamp();

    for (id, text) in [("EvT1", "one"), ("EvT2", "two")] {
        let body = mention_event(id, "UINSTALL", "C9", "9.1", Some("9.0"), text);
        let (_, sig) = signed_request(&body, now);
        post_slack_event(app.clone(), &body, now, &sig).await;
    }
    let body = mention_event("EvT3", "UINSTALL", "C9", "10.0", None, "other thread");
    let (_, sig) = signed_request(&body, now);
    post_slack_event(app.clone(), &body, now, &sig).await;

    for (id, text) in [("EvD1", "dm one"), ("EvD2", "dm two")] {
        let body = dm_event(id, "UINSTALL", "DSTABLE", text);
        let (_, sig) = signed_request(&body, now);
        post_slack_event(app.clone(), &body, now, &sig).await;
    }

    let convos: Vec<(String, String)> = sqlx::query_as(
        "SELECT external_thread_id, conversation_id FROM channel_threads ORDER BY created_at",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(convos.len(), 3);
    let thread_a = &convos.iter().find(|(t, _)| t == "9.0").unwrap().1;
    let thread_b = &convos.iter().find(|(t, _)| t == "10.0").unwrap().1;
    let dm = &convos.iter().find(|(t, _)| t == "DSTABLE").unwrap().1;
    assert_ne!(thread_a, thread_b);
    assert_ne!(thread_a, dm);

    let same_thread_runs: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT conversation_id) FROM agent_runs WHERE conversation_id = $1",
    )
    .bind(thread_a)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(same_thread_runs, 1);
    let dm_runs: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs WHERE conversation_id = $1")
            .bind(dm)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(dm_runs, 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn slack_work_uses_normal_admission_path(pool: PgPool) {
    let bot = insert_bot(&pool, OWNER).await;
    connect_slack(&pool, OWNER, &bot.id, "T1", "UINSTALL").await;
    let web = work::enqueue(&pool, OWNER, "web-1", &bot.id, None, "hello web")
        .await
        .unwrap();

    let state = AppState::new(pool.clone(), test_config(None));
    let app = build_router(state);
    let now = chrono::Utc::now().timestamp();
    let body = dm_event("EvPATH", "UINSTALL", "DPATH", "hello slack");
    let (_, sig) = signed_request(&body, now);
    post_slack_event(app, &body, now, &sig).await;

    let slack_run = sqlx::query(
        "SELECT id, computer_id, model, origin_kind, origin_provider FROM agent_runs WHERE request_id LIKE 'slack:%' LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let computer: String = sqlx::Row::get(&slack_run, "computer_id");
    let model: String = sqlx::Row::get(&slack_run, "model");
    let origin_kind: String = sqlx::Row::get(&slack_run, "origin_kind");
    let origin_provider: Option<String> = sqlx::Row::get(&slack_run, "origin_provider");
    assert_eq!(computer, web.computer_id);
    assert_eq!(model, web.model);
    assert_eq!(origin_kind, "channel");
    assert_eq!(origin_provider.as_deref(), Some(PROVIDER_SLACK));
    assert_ne!(sqlx::Row::get::<String, _>(&slack_run, "id"), web.run_id);

    let web_instructions: String =
        sqlx::query_scalar("SELECT instructions FROM work_queue WHERE run_id = $1")
            .bind(&web.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let slack_instructions: String =
        sqlx::query_scalar("SELECT instructions FROM work_queue WHERE run_id = $1")
            .bind(sqlx::Row::get::<String, _>(&slack_run, "id"))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(web_instructions, slack_instructions);
    assert!(slack_instructions.contains("You are \"Scout\""));
}

#[sqlx::test(migrations = "./migrations")]
async fn slack_codex_thread_stays_on_mapped_conversation(pool: PgPool) {
    let bot = insert_bot(&pool, OWNER).await;
    connect_slack(&pool, OWNER, &bot.id, "T1", "UINSTALL").await;
    let state = AppState::new(pool.clone(), test_config(None));
    let app = build_router(state);
    let now = chrono::Utc::now().timestamp();
    let body = dm_event("EvCX1", "UINSTALL", "DCX", "first");
    let (_, sig) = signed_request(&body, now);
    post_slack_event(app.clone(), &body, now, &sig).await;
    let conversation_id: String = sqlx::query_scalar(
        "SELECT conversation_id FROM channel_threads WHERE external_channel_id = 'DCX'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    conversation::set_codex_thread_id(&pool, &conversation_id, &bot.id, "codex-thread-1")
        .await
        .unwrap();
    let body = dm_event("EvCX2", "UINSTALL", "DCX", "second");
    let (_, sig) = signed_request(&body, now);
    post_slack_event(app, &body, now, &sig).await;
    let again: String = sqlx::query_scalar(
        "SELECT conversation_id FROM channel_threads WHERE external_channel_id = 'DCX'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(conversation_id, again);
    assert_eq!(
        conversation::get_codex_thread_id(&pool, &conversation_id, &bot.id)
            .await
            .unwrap()
            .as_deref(),
        Some("codex-thread-1")
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn delivery_outbox_retry_and_restart(pool: PgPool) {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat.postMessage"))
        .and(header("authorization", "Bearer xoxb-test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ok": true,
            "ts": "171.1"
        })))
        .mount(&mock)
        .await;

    let bot = insert_bot(&pool, OWNER).await;
    connect_slack(&pool, OWNER, &bot.id, "T1", "UINSTALL").await;
    let state = AppState::new(pool.clone(), test_config(Some(mock.uri())));
    let app = build_router(state.clone());
    let now = chrono::Utc::now().timestamp();
    let body = dm_event("EvDEL", "UINSTALL", "DDEL", "please reply");
    let (_, sig) = signed_request(&body, now);
    post_slack_event(app, &body, now, &sig).await;

    let run_id: String =
        sqlx::query_scalar("SELECT id FROM agent_runs WHERE request_id LIKE 'slack:%'")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("UPDATE messages SET body = 'final answer', status = 'complete' WHERE conversation_id IN (SELECT conversation_id FROM agent_runs WHERE id = $1) AND role = 'assistant'")
        .bind(&run_id)
        .execute(&pool)
        .await
        .unwrap();
    let inserted = delivery::enqueue_assistant_reply_for_run_pool(&pool, &run_id, None)
        .await
        .unwrap();
    assert_eq!(inserted, 1);
    let again = delivery::enqueue_assistant_reply_for_run_pool(&pool, &run_id, None)
        .await
        .unwrap();
    assert_eq!(again, 0);

    assert_eq!(delivery::tick(&state).await.unwrap(), 1);
    let status: String =
        sqlx::query_scalar("SELECT status FROM channel_deliveries WHERE source_run_id = $1")
            .bind(&run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "sent");
    let ts: Option<String> = sqlx::query_scalar(
        "SELECT provider_message_id FROM channel_deliveries WHERE source_run_id = $1",
    )
    .bind(&run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ts.as_deref(), Some("171.1"));

    sqlx::query("UPDATE channel_deliveries SET status = 'sending' WHERE source_run_id = $1")
        .bind(&run_id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(delivery::recover_sending(&pool).await.unwrap(), 1);
    let recovered: String =
        sqlx::query_scalar("SELECT status FROM channel_deliveries WHERE source_run_id = $1")
            .bind(&run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(recovered, "retryable");
}

#[sqlx::test(migrations = "./migrations")]
async fn delivery_honors_retry_after_and_permanent_auth_failure(pool: PgPool) {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat.postMessage"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "7")
                .set_body_json(json!({"ok": false, "error": "ratelimited"})),
        )
        .mount(&mock)
        .await;

    let bot = insert_bot(&pool, OWNER).await;
    connect_slack(&pool, OWNER, &bot.id, "T1", "UINSTALL").await;
    let run = work::enqueue(&pool, OWNER, "retry-run", &bot.id, None, "retry me")
        .await
        .unwrap();
    let state = AppState::new(pool.clone(), test_config(Some(mock.uri())));
    let thread_id = Uuid::new_v4().to_string();
    let connection_id: String =
        sqlx::query_scalar("SELECT id FROM channel_connections WHERE owner_id = $1")
            .bind(OWNER)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("UPDATE conversations SET origin_kind = 'channel' WHERE id = $1")
        .bind(&run.conversation_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO channel_threads (id, connection_id, owner_id, bot_id, provider, external_channel_id, external_thread_id, conversation_id) VALUES ($1,$2,$3,$4,'slack','C429','9.0',$5)",
    )
    .bind(&thread_id)
    .bind(&connection_id)
    .bind(OWNER)
    .bind(&bot.id)
    .bind(&run.conversation_id)
    .execute(&pool)
    .await
    .unwrap();
    let delivery_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO channel_deliveries (id, connection_id, thread_id, owner_id, source_run_id, kind, chunk_index, body, status) VALUES ($1,$2,$3,$4,$5,'owner_attention',0,'hello','queued')",
    )
    .bind(&delivery_id)
    .bind(&connection_id)
    .bind(&thread_id)
    .bind(OWNER)
    .bind(&run.run_id)
    .execute(&pool)
    .await
    .unwrap();

    delivery::tick(&state).await.unwrap();
    let (status, retry_at): (String, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as("SELECT status, next_attempt_at FROM channel_deliveries WHERE id = $1")
            .bind(&delivery_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "retryable");
    let wait = retry_at.unwrap() - chrono::Utc::now();
    assert!(wait.num_seconds() >= 6);

    mock.reset().await;
    Mock::given(method("POST"))
        .and(path("/chat.postMessage"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ok": false,
            "error": "invalid_auth"
        })))
        .mount(&mock)
        .await;
    sqlx::query(
        "UPDATE channel_deliveries SET status = 'queued', next_attempt_at = NOW() WHERE id = $1",
    )
    .bind(&delivery_id)
    .execute(&pool)
    .await
    .unwrap();
    delivery::tick(&state).await.unwrap();
    let status: String = sqlx::query_scalar("SELECT status FROM channel_deliveries WHERE id = $1")
        .bind(&delivery_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "failed");
}

#[sqlx::test(migrations = "./migrations")]
async fn owner_attention_notice_is_one_shot_and_not_an_approval_channel(pool: PgPool) {
    let bot = insert_bot(&pool, OWNER).await;
    connect_slack(&pool, OWNER, &bot.id, "T1", "UINSTALL").await;
    let web = work::enqueue(&pool, OWNER, "att-1", &bot.id, None, "need tools")
        .await
        .unwrap();
    sqlx::query(
        "UPDATE agent_runs SET origin_kind = 'channel', origin_provider = 'slack' WHERE id = $1",
    )
    .bind(&web.run_id)
    .execute(&pool)
    .await
    .unwrap();
    let conversation_id = web.conversation_id.clone();
    let connection_id: String =
        sqlx::query_scalar("SELECT id FROM channel_connections WHERE owner_id = $1")
            .bind(OWNER)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("UPDATE conversations SET origin_kind = 'channel' WHERE id = $1")
        .bind(&conversation_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO channel_threads (id, connection_id, owner_id, bot_id, provider, external_channel_id, external_thread_id, conversation_id) VALUES ($1,$2,$3,$4,'slack','CATT','1.0',$5)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&connection_id)
    .bind(OWNER)
    .bind(&bot.id)
    .bind(&conversation_id)
    .execute(&pool)
    .await
    .unwrap();

    assert!(delivery::enqueue_owner_attention(
        &pool,
        &web.run_id,
        "approval",
        Some("http://localhost:3000")
    )
    .await
    .unwrap());
    assert!(!delivery::enqueue_owner_attention(
        &pool,
        &web.run_id,
        "approval",
        Some("http://localhost:3000")
    )
    .await
    .unwrap());
    let body: String = sqlx::query_scalar(
        "SELECT body FROM channel_deliveries WHERE source_run_id = $1 AND kind = 'owner_attention'",
    )
    .bind(&web.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(body.contains("needs your approval in Elsewhere"));
    assert!(body.contains("/app/work/"));
    assert!(!body.contains("xoxb-"));

    let help = work::enqueue(&pool, OWNER, "att-help", &bot.id, None, "need help")
        .await
        .unwrap();
    sqlx::query("UPDATE agent_runs SET origin_kind = 'channel', origin_provider = 'slack', conversation_id = $2 WHERE id = $1")
        .bind(&help.run_id)
        .bind(&conversation_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(delivery::enqueue_owner_attention(
        &pool,
        &help.run_id,
        "help",
        Some("http://localhost:3000")
    )
    .await
    .unwrap());
    let help_body: String = sqlx::query_scalar(
        "SELECT body FROM channel_deliveries WHERE source_run_id = $1 AND kind = 'owner_attention'",
    )
    .bind(&help.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(help_body.contains("needs your help in Elsewhere"));

    let state = AppState::new(pool.clone(), test_config(None));
    let app = build_router(state);
    let now = chrono::Utc::now().timestamp();
    let fake_button = serde_json::to_string(&json!({
        "type": "event_callback",
        "team_id": "T1",
        "event_id": "EvBUTTON",
        "event": {"type": "block_actions", "user": "UINSTALL", "actions": [{"action_id": "approve"}]}
    }))
    .unwrap();
    let (_, sig) = signed_request(&fake_button, now);
    let (status, _) = post_slack_event(app, &fake_button, now, &sig).await;
    assert_eq!(status, http::StatusCode::OK);
    let approvals: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tool_approval_requests")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);
    assert_eq!(approvals, 0);
}

#[test]
fn slack_tokens_are_redacted() {
    let msg = redact_secrets("failed xoxb-abc SLACK_SIGNING_SECRET leaked");
    assert!(!msg.contains("xoxb-abc"));
    assert!(!msg.contains("SLACK_SIGNING_SECRET"));
}

#[test]
fn outbound_body_is_bounded() {
    let long = "a\n".repeat(5000);
    let chunks = split_outbound_body(&long, Some("http://localhost:3000/app/work/r1"));
    assert!(chunks.len() <= 3);
    assert!(chunks.iter().all(|c| c.chars().count() <= 4000));
}
