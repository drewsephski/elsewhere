//! Hosted bot runs: `build_computer` → `connect_agent_computer` → `RemoteLocalMacComputer`.

mod support;

use agent_core::{
    CreateResponseResult, FakeAgentComputer, HostToMacMessage, LocalMacRpcDispatcher,
    MacToHostMessage, ModelError, ResponsesModel,
};
use async_trait::async_trait;
use cloud_host::auth::{JwtVerifier, JwtVerifierConfig};
use cloud_host::config::AuthMode;
use cloud_host::db::resources::insert_bot;
use cloud_host::{build_router, test_signing, AppState, TestRunOverrides};
use futures_util::{SinkExt, StreamExt};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::AUTHORIZATION;
use tokio_tungstenite::tungstenite::Message;
use tower::ServiceExt;
use uuid::Uuid;

use support::test_config;

const PAIRING_TEST_KEY: [u8; 32] = [9u8; 32];
const TEST_JWT_ISSUER: &str = "http://localhost:3000";
const TEST_JWT_AUDIENCE: &str = "elsewhere-cloud-host";

fn pairing_test_config() -> cloud_host::Config {
    let mut config = test_config();
    config.auth_mode = AuthMode::Hybrid;
    config.jwt_issuer = Some(TEST_JWT_ISSUER.into());
    config.jwt_audience = Some(TEST_JWT_AUDIENCE.into());
    config.jwt_jwks_url = Some("http://127.0.0.1:9/jwks".into());
    config.cors_web_origin = Some("http://localhost:3000".into());
    config.local_mac_credential_key = Some(PAIRING_TEST_KEY);
    config.enforce_tool_approvals_internal = true;
    config.legacy_local_approval_bypass = false;
    config
}

fn jwt_state(pool: PgPool) -> AppState {
    let mut state = AppState::new(pool, pairing_test_config());
    state.jwt_verifier = Some(JwtVerifier::from_test_decoding_key(
        test_signing::TEST_KID,
        test_signing::verifier(),
        JwtVerifierConfig {
            jwks_url: "http://127.0.0.1:9/jwks".into(),
            issuer: TEST_JWT_ISSUER.into(),
            audience: TEST_JWT_AUDIENCE.into(),
        },
    ));
    state
}

fn token(sub: &str) -> String {
    test_signing::user_token(sub, TEST_JWT_ISSUER, TEST_JWT_AUDIENCE, 300)
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| json!({ "raw": String::from_utf8_lossy(&bytes) }))
}

async fn post_json(
    app: &axum::Router,
    uri: &str,
    auth: Option<&str>,
    body: Value,
) -> axum::response::Response {
    let mut builder = http::Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(tok) = auth {
        builder = builder.header("authorization", format!("Bearer {tok}"));
    }
    app.clone()
        .oneshot(
            builder
                .body(axum::body::Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

struct IssuedNode {
    owner: String,
    node_id: String,
    computer_id: String,
    credential: String,
}

async fn issue_node(app: &axum::Router, device_name: &str) -> IssuedNode {
    let installation_id = Uuid::new_v4().to_string();
    let started = json_body(
        post_json(
            app,
            "/v1/local-mac/pairings",
            None,
            json!({
                "deviceName": device_name,
                "installationId": installation_id
            }),
        )
        .await,
    )
    .await;
    let pairing_id = started["pairingId"].as_str().unwrap();
    let owner = format!("owner-{}", Uuid::new_v4());
    let approve = post_json(
        app,
        &format!("/v1/local-mac/pairings/{pairing_id}/approve"),
        Some(&token(&owner)),
        json!({ "userCode": started["userCode"] }),
    )
    .await;
    assert_eq!(approve.status(), http::StatusCode::OK);
    let issued = json_body(
        post_json(
            app,
            &format!("/v1/local-mac/pairings/{pairing_id}/exchange"),
            None,
            json!({ "pairingSecret": started["pairingSecret"] }),
        )
        .await,
    )
    .await;
    IssuedNode {
        owner,
        node_id: issued["nodeId"].as_str().unwrap().to_string(),
        computer_id: issued["computerId"].as_str().unwrap().to_string(),
        credential: issued["credential"].as_str().unwrap().to_string(),
    }
}

struct ScriptedModel {
    steps: Mutex<Vec<CreateResponseResult>>,
}

#[async_trait]
impl ResponsesModel for ScriptedModel {
    async fn create_response(
        &self,
        _request: agent_core::CreateResponseRequest,
    ) -> Result<CreateResponseResult, ModelError> {
        let mut steps = self.steps.lock().unwrap();
        if steps.is_empty() {
            return Ok(CreateResponseResult {
                output: vec![json!({
                    "type": "message",
                    "content": [{"type":"output_text","text":"done"}]
                })],
                output_text: Some("done".into()),
            });
        }
        Ok(steps.remove(0))
    }
}

fn write_then_done_model() -> Arc<ScriptedModel> {
    Arc::new(ScriptedModel {
        steps: Mutex::new(vec![
            CreateResponseResult {
                output: vec![json!({
                    "type": "function_call",
                    "name": "workspace_write",
                    "call_id": "c1",
                    "arguments": "{\"path\":\"/workspace/a.txt\",\"content\":\"hi\"}"
                })],
                output_text: None,
            },
            CreateResponseResult {
                output: vec![json!({
                    "type": "message",
                    "content": [{"type":"output_text","text":"done"}]
                })],
                output_text: Some("done".into()),
            },
        ]),
    })
}

struct ModelOnlyGuard {
    state: AppState,
    request_id: String,
}

impl ModelOnlyGuard {
    fn install(state: AppState, request_id: String, model: Arc<ScriptedModel>) -> Self {
        state.register_test_run_overrides(
            &request_id,
            TestRunOverrides {
                computer: None,
                model,
            },
        );
        Self { state, request_id }
    }
}

impl Drop for ModelOnlyGuard {
    fn drop(&mut self) {
        self.state.clear_test_run_overrides(&self.request_id);
    }
}

async fn start_http_server(state: AppState) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    tokio::spawn(async move {
        axum::serve(listener, build_router(state)).await.unwrap();
    });
    addr
}

async fn connect_fake_mac_bridge(ws_addr: &str, issued: &IssuedNode, fake: Arc<FakeAgentComputer>) {
    let mut request = format!("ws://{ws_addr}/v1/local-mac/sessions")
        .into_client_request()
        .unwrap();
    request.headers_mut().insert(
        AUTHORIZATION,
        format!("Bearer {}", issued.credential).parse().unwrap(),
    );
    let (ws, _) = tokio_tungstenite::connect_async(request).await.unwrap();
    let (mut sink, mut stream) = ws.split();
    sink.send(Message::Text(
        serde_json::to_string(&MacToHostMessage::Hello {
            protocol_version: 1,
            node_id: issued.node_id.clone(),
            computer_id: issued.computer_id.clone(),
        })
        .unwrap()
        .into(),
    ))
    .await
    .unwrap();
    let _ack = stream.next().await.unwrap().unwrap();

    let dispatcher = Arc::new(LocalMacRpcDispatcher::new(fake.clone()));
    tokio::spawn(async move {
        while let Some(Ok(message)) = stream.next().await {
            match message {
                Message::Text(text) => {
                    if let Ok(HostToMacMessage::Rpc {
                        id,
                        protocol_version,
                        method,
                        params,
                        ..
                    }) = serde_json::from_str(&text)
                    {
                        let result = dispatcher
                            .handle_rpc(id, protocol_version, method, params)
                            .await;
                        sink.send(Message::Text(
                            serde_json::to_string(&result).unwrap().into(),
                        ))
                        .await
                        .ok();
                    }
                }
                Message::Ping(payload) => {
                    sink.send(Message::Pong(payload)).await.ok();
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(80)).await;
}

async fn seed_local_mac_bot(pool: &PgPool, owner: &str, computer_id: &str) -> String {
    let bot = insert_bot(
        pool,
        owner,
        "Mac Bot",
        "test",
        "gpt-5.6-luna",
        Some(computer_id),
        "responses",
        "sky-wisp",
    )
    .await
    .unwrap();
    bot.id
}

async fn wait_pending_approval(pool: &PgPool, owner: &str) -> String {
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM tool_approval_requests WHERE owner_id = $1 AND status = 'pending' ORDER BY requested_at DESC LIMIT 1",
        )
        .bind(owner)
        .fetch_optional(pool)
        .await
        .unwrap();
        if let Some((id,)) = row {
            return id;
        }
    }
    panic!("timed out waiting for pending approval");
}

async fn wait_run_status(pool: &PgPool, request_id: &str, want: &str) -> bool {
    for _ in 0..120 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let status: Option<String> =
            sqlx::query_scalar("SELECT status FROM agent_runs WHERE request_id = $1")
                .bind(request_id)
                .fetch_optional(pool)
                .await
                .unwrap();
        if status.as_deref() == Some(want) {
            return true;
        }
    }
    false
}

async fn start_hosted_run(
    app: &axum::Router,
    state: &AppState,
    owner: &str,
    bot_id: &str,
    request_id: &str,
    model: Arc<ScriptedModel>,
) -> ModelOnlyGuard {
    let guard = ModelOnlyGuard::install(state.clone(), request_id.to_string(), model);
    let resp = app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("POST")
                .uri("/v1/runs")
                .header("Authorization", format!("Bearer {}", token(owner)))
                .header("Idempotency-Key", request_id)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({ "botId": bot_id, "message": "write" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::ACCEPTED);
    cloud_host::worker::dispatch_available(state).await.unwrap();
    guard
}

#[sqlx::test(migrations = "./migrations")]
async fn local_mac_hosted_run_approval_before_mac_write_rpc(pool: PgPool) {
    let state = jwt_state(pool.clone());
    let app = build_router(state.clone());
    let ws_addr = start_http_server(state.clone()).await;
    let issued = issue_node(&app, "Routing Mac").await;
    let fake = Arc::new(FakeAgentComputer::new());
    connect_fake_mac_bridge(&ws_addr, &issued, fake.clone()).await;

    assert_eq!(state.computer_registry.cached_sprite_count(), 0);

    let bot_id = seed_local_mac_bot(&pool, &issued.owner, &issued.computer_id).await;
    let request_id = Uuid::new_v4().to_string();
    let _guard = start_hosted_run(
        &app,
        &state,
        &issued.owner,
        &bot_id,
        &request_id,
        write_then_done_model(),
    )
    .await;

    let snapshotted: String = sqlx::query_scalar(
        "SELECT computer_id FROM agent_runs WHERE request_id = $1 AND owner_id = $2",
    )
    .bind(&request_id)
    .bind(&issued.owner)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(snapshotted, issued.computer_id);

    let approval_id = wait_pending_approval(&pool, &issued.owner).await;
    assert_eq!(fake.write_file_calls(), 0);
    assert_eq!(state.computer_registry.cached_sprite_count(), 0);

    let approve = app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("POST")
                .uri(format!("/v1/approvals/{approval_id}/approve"))
                .header("Authorization", format!("Bearer {}", token(&issued.owner)))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(approve.status(), http::StatusCode::OK);

    for _ in 0..80 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if fake.write_file_calls() == 1 {
            break;
        }
    }
    assert_eq!(fake.write_file_calls(), 1);
    assert!(wait_run_status(&pool, &request_id, "completed").await);
    assert_eq!(state.computer_registry.cached_sprite_count(), 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn local_mac_hosted_run_deny_skips_mac_write_rpc(pool: PgPool) {
    let state = jwt_state(pool.clone());
    let app = build_router(state.clone());
    let ws_addr = start_http_server(state.clone()).await;
    let issued = issue_node(&app, "Deny Mac").await;
    let fake = Arc::new(FakeAgentComputer::new());
    connect_fake_mac_bridge(&ws_addr, &issued, fake.clone()).await;

    let bot_id = seed_local_mac_bot(&pool, &issued.owner, &issued.computer_id).await;
    let request_id = Uuid::new_v4().to_string();
    let _guard = start_hosted_run(
        &app,
        &state,
        &issued.owner,
        &bot_id,
        &request_id,
        write_then_done_model(),
    )
    .await;

    let approval_id = wait_pending_approval(&pool, &issued.owner).await;
    assert_eq!(fake.write_file_calls(), 0);

    let deny = app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("POST")
                .uri(format!("/v1/approvals/{approval_id}/deny"))
                .header("Authorization", format!("Bearer {}", token(&issued.owner)))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deny.status(), http::StatusCode::OK);

    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_eq!(fake.write_file_calls(), 0);
    assert_eq!(state.computer_registry.cached_sprite_count(), 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn local_mac_offline_active_computer_never_opens_sprite(pool: PgPool) {
    use agent_core::{ComputerError, LOCAL_MAC_NOT_CONNECTED};

    let state = jwt_state(pool.clone());
    let app = build_router(state.clone());
    let issued = issue_node(&app, "Offline Mac").await;
    sqlx::query(
        "UPDATE sandboxes SET state = 'active', last_used_at = NOW(), updated_at = NOW() WHERE id = $1",
    )
    .bind(&issued.computer_id)
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(state.computer_registry.cached_sprite_count(), 0);
    let computer = state
        .computer_registry
        .connect_agent_computer(
            &state.config,
            &state.pool,
            &issued.owner,
            &issued.computer_id,
            &state.local_mac_sessions,
            false,
        )
        .await
        .unwrap();
    let err = computer.ensure_ready().await.unwrap_err();
    assert_eq!(
        err,
        ComputerError::GuestUnavailable(LOCAL_MAC_NOT_CONNECTED.into())
    );
    assert_eq!(state.computer_registry.cached_sprite_count(), 0);
    assert!(!state
        .local_mac_sessions
        .is_connected(&issued.owner, &issued.computer_id));
}
