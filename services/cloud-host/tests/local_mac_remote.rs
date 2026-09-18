//! Credential authentication, WebSocket sessions, and provider-neutral resolve.

mod support;

use agent_core::{
    FakeAgentComputer, HostToMacMessage, LocalMacRpcDispatcher, MacToHostMessage,
    LOCAL_MAC_NOT_CONNECTED,
};
use cloud_host::auth::{JwtVerifier, JwtVerifierConfig};
use cloud_host::db::resources::insert_computer_placeholder;
use cloud_host::local_mac::db::{
    self, active_device_credential_exists, authenticate_device_session,
};
use cloud_host::local_mac::session::RemoteLocalMacComputer;
use cloud_host::{build_router, test_signing, AppState};
use futures_util::{SinkExt, StreamExt};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::AUTHORIZATION;
use tokio_tungstenite::tungstenite::Message;
use tower::ServiceExt;
use uuid::Uuid;

use support::test_config;

const PAIRING_TEST_KEY: [u8; 32] = [7u8; 32];
const TEST_JWT_ISSUER: &str = "http://localhost:3000";
const TEST_JWT_AUDIENCE: &str = "elsewhere-cloud-host";

fn pairing_test_config() -> cloud_host::Config {
    let mut config = test_config();
    config.auth_mode = cloud_host::config::AuthMode::Hybrid;
    config.jwt_issuer = Some(TEST_JWT_ISSUER.into());
    config.jwt_audience = Some(TEST_JWT_AUDIENCE.into());
    config.jwt_jwks_url = Some("http://127.0.0.1:9/jwks".into());
    config.cors_web_origin = Some("http://localhost:3000".into());
    config.local_mac_credential_key = Some(PAIRING_TEST_KEY);
    config
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

fn pairing_state(pool: PgPool) -> AppState {
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
    if let Some(token) = auth {
        builder = builder.header("authorization", format!("Bearer {token}"));
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

async fn get_auth(app: &axum::Router, uri: &str, token: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            http::Request::builder()
                .uri(uri)
                .header("authorization", format!("Bearer {token}"))
                .body(axum::body::Body::empty())
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
    let start = post_json(
        app,
        "/v1/local-mac/pairings",
        None,
        json!({
            "deviceName": device_name,
            "installationId": installation_id
        }),
    )
    .await;
    assert_eq!(start.status(), http::StatusCode::OK);
    let started = json_body(start).await;
    let pairing_id = started["pairingId"].as_str().unwrap().to_string();
    let owner = format!("owner-{}", Uuid::new_v4());
    let approve = post_json(
        app,
        &format!("/v1/local-mac/pairings/{pairing_id}/approve"),
        Some(&token(&owner)),
        json!({ "userCode": started["userCode"] }),
    )
    .await;
    assert_eq!(approve.status(), http::StatusCode::OK);
    let exchange = post_json(
        app,
        &format!("/v1/local-mac/pairings/{pairing_id}/exchange"),
        None,
        json!({ "pairingSecret": started["pairingSecret"] }),
    )
    .await;
    assert_eq!(exchange.status(), http::StatusCode::OK);
    let issued = json_body(exchange).await;
    IssuedNode {
        owner,
        node_id: issued["nodeId"].as_str().unwrap().to_string(),
        computer_id: issued["computerId"].as_str().unwrap().to_string(),
        credential: issued["credential"].as_str().unwrap().to_string(),
    }
}

#[tokio::test]
async fn credential_auth_accepts_valid_and_rejects_mismatches() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping credential_auth_accepts_valid_and_rejects_mismatches");
        return;
    };
    let app = build_router(pairing_state(pool.clone()));
    let a = issue_node(&app, "Auth Mac A").await;
    let b = issue_node(&app, "Auth Mac B").await;

    let ok = authenticate_device_session(
        &pool,
        &PAIRING_TEST_KEY,
        &a.credential,
        &a.node_id,
        &a.computer_id,
    )
    .await
    .unwrap();
    assert_eq!(ok.owner_id, a.owner);
    assert_eq!(ok.node_id, a.node_id);
    assert_eq!(ok.computer_id, a.computer_id);
    assert!(
        active_device_credential_exists(&pool, &PAIRING_TEST_KEY, &a.credential)
            .await
            .unwrap()
    );

    assert!(authenticate_device_session(
        &pool,
        &PAIRING_TEST_KEY,
        "emac_this-is-not-the-credential",
        &a.node_id,
        &a.computer_id,
    )
    .await
    .is_err());

    assert!(authenticate_device_session(
        &pool,
        &PAIRING_TEST_KEY,
        &a.credential,
        &b.node_id,
        &b.computer_id,
    )
    .await
    .is_err());

    assert!(authenticate_device_session(
        &pool,
        &PAIRING_TEST_KEY,
        &a.credential,
        &a.node_id,
        &b.computer_id,
    )
    .await
    .is_err());

    db::revoke_node(&pool, &a.owner, &a.node_id).await.unwrap();
    assert!(
        !active_device_credential_exists(&pool, &PAIRING_TEST_KEY, &a.credential)
            .await
            .unwrap()
    );
    assert!(authenticate_device_session(
        &pool,
        &PAIRING_TEST_KEY,
        &a.credential,
        &a.node_id,
        &a.computer_id,
    )
    .await
    .is_err());

    sqlx::query("UPDATE sandboxes SET state = 'archived' WHERE id = $1")
        .bind(&b.computer_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(authenticate_device_session(
        &pool,
        &PAIRING_TEST_KEY,
        &b.credential,
        &b.node_id,
        &b.computer_id,
    )
    .await
    .is_err());

    sqlx::query("DELETE FROM sandboxes WHERE id = $1")
        .bind(&b.computer_id)
        .execute(&pool)
        .await
        .ok();
}

#[tokio::test]
async fn websocket_session_and_remote_computer_over_real_transport() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping websocket_session_and_remote_computer_over_real_transport");
        return;
    };
    let state = pairing_state(pool.clone());
    let issued = {
        let app = build_router(state.clone());
        issue_node(&app, "WS Mac").await
    };

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server_state = state.clone();
    tokio::spawn(async move {
        axum::serve(listener, build_router(server_state))
            .await
            .unwrap();
    });

    let mut request = format!("ws://{addr}/v1/local-mac/sessions")
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
    let ack = stream.next().await.unwrap().unwrap();
    let Message::Text(text) = ack else {
        panic!("expected hello_ack text");
    };
    let parsed: HostToMacMessage = serde_json::from_str(&text).unwrap();
    assert!(matches!(parsed, HostToMacMessage::HelloAck { .. }));

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(state
        .local_mac_sessions
        .is_connected(&issued.owner, &issued.computer_id));

    let listed = {
        let app = build_router(state.clone());
        get_auth(&app, "/v1/computers", &token(&issued.owner)).await
    };
    let computers = json_body(listed).await;
    let found = computers
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == issued.computer_id)
        .unwrap();
    assert_eq!(found["providerMetadata"]["connected"], true);
    assert_eq!(found["state"], "active");

    let fake = Arc::new(FakeAgentComputer::new());
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
                        .unwrap();
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
    let proof = cloud_host::local_mac::session::run_workspace_rpc_proof(computer.as_ref())
        .await
        .unwrap();
    assert!(proof.contains("hello from RemoteLocalMacComputer"));
    assert_eq!(fake.write_file_calls(), 1);

    let wrong = tokio_tungstenite::connect_async({
        let mut request = format!("ws://{addr}/v1/local-mac/sessions")
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert(AUTHORIZATION, "Bearer emac_not-valid".parse().unwrap());
        request
    })
    .await;
    assert!(wrong.is_err());
}

#[tokio::test]
async fn connect_agent_computer_resolves_local_mac_and_rejects_unknown() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping connect_agent_computer_resolves_local_mac_and_rejects_unknown");
        return;
    };
    let state = pairing_state(pool.clone());
    let app = build_router(state.clone());
    let issued = issue_node(&app, "Resolver Mac").await;

    let computer = state
        .computer_registry
        .connect_agent_computer(
            &state.config,
            &state.pool,
            &issued.owner,
            &issued.computer_id,
            &state.local_mac_sessions,
            true,
        )
        .await
        .unwrap();
    let err = computer.ensure_ready().await.unwrap_err();
    assert_eq!(
        err,
        agent_core::ComputerError::GuestUnavailable(LOCAL_MAC_NOT_CONNECTED.into())
    );
    let browser = computer
        .browser_invoke("navigate", &json!({}))
        .await
        .unwrap_err();
    assert!(matches!(
        browser,
        agent_core::ComputerError::SandboxRejected(_)
    ));
    let _ = RemoteLocalMacComputer::new(
        issued.owner.clone(),
        issued.computer_id.clone(),
        issued.node_id.clone(),
        state.local_mac_sessions.clone(),
    );

    let sprite = insert_computer_placeholder(&pool, &issued.owner, "Cloud box")
        .await
        .unwrap();
    sqlx::query("UPDATE sandboxes SET provider = 'mystery' WHERE id = $1")
        .bind(&sprite.id)
        .execute(&pool)
        .await
        .unwrap();
    let unsupported = state
        .computer_registry
        .connect_agent_computer(
            &state.config,
            &state.pool,
            &issued.owner,
            &sprite.id,
            &state.local_mac_sessions,
            false,
        )
        .await
        .err()
        .expect("mystery provider should be unsupported");
    assert!(unsupported
        .to_string()
        .contains("unsupported computer provider"));
}

#[tokio::test]
async fn connect_sprite_still_rejects_local_mac() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping connect_sprite_still_rejects_local_mac");
        return;
    };
    let state = pairing_state(pool.clone());
    let app = build_router(state.clone());
    let issued = issue_node(&app, "Sprite Reject Mac").await;
    let err = state
        .computer_registry
        .connect_sprite(
            &state.config,
            &state.pool,
            &issued.owner,
            &issued.computer_id,
            false,
        )
        .await
        .err()
        .expect("local_mac must not use SpriteComputer");
    assert!(err
        .to_string()
        .contains("not available for hosted Sprite operations"));
}
