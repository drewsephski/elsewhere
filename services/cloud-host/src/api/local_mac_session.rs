//! Authenticated outbound Mac session WebSocket.
//!
//! Route: `GET /v1/local-mac/sessions` on cloud-host (public runner HTTP, same
//! origin as `/health`). The Mac connects with `Authorization: Bearer emac_...`
//! then sends a v1 `hello`. This is not a generic Elsewhere API credential.

use agent_core::{HostToMacMessage, MacToHostMessage, PROTOCOL_VERSION};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use crate::app_state::AppState;
use crate::local_mac::db::{
    active_device_credential_exists, authenticate_device_session, touch_node_connected,
};
use crate::local_mac::require_credential_key;
use crate::local_mac::session::LocalMacIdentity;

const HELLO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const PING_INTERVAL: std::time::Duration = std::time::Duration::from_secs(20);

pub async fn local_mac_session(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> axum::response::Response {
    let credential = match bearer_credential(&headers) {
        Some(value) => value,
        None => {
            return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
        }
    };
    let key = match require_credential_key(state.config.local_mac_credential_key.as_ref()) {
        Ok(key) => *key,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "local Mac pairing is not configured",
            )
                .into_response();
        }
    };
    match active_device_credential_exists(&state.pool, &key, &credential).await {
        Ok(true) => {}
        Ok(false) | Err(_) => {
            return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
        }
    }
    ws.on_upgrade(move |socket| handle_socket(state, socket, credential))
        .into_response()
}

fn bearer_credential(headers: &HeaderMap) -> Option<String> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    let token = value.strip_prefix("Bearer ")?.trim();
    if token.starts_with("emac_") {
        Some(token.to_string())
    } else {
        None
    }
}

async fn handle_socket(state: AppState, mut socket: WebSocket, credential: String) {
    let hello = match tokio::time::timeout(HELLO_TIMEOUT, recv_hello(&mut socket)).await {
        Ok(Ok(hello)) => hello,
        _ => {
            let _ = socket
                .send(Message::Close(Some(axum::extract::ws::CloseFrame {
                    code: axum::extract::ws::close_code::POLICY,
                    reason: "hello required".into(),
                })))
                .await;
            return;
        }
    };

    let key = match require_credential_key(state.config.local_mac_credential_key.as_ref()) {
        Ok(key) => *key,
        Err(_) => return,
    };
    let authenticated = match authenticate_device_session(
        &state.pool,
        &key,
        &credential,
        &hello.node_id,
        &hello.computer_id,
    )
    .await
    {
        Ok(identity) => identity,
        Err(_) => {
            tracing::info!("local Mac session rejected");
            let _ = socket
                .send(Message::Close(Some(axum::extract::ws::CloseFrame {
                    code: axum::extract::ws::close_code::POLICY,
                    reason: "unauthorized".into(),
                })))
                .await;
            return;
        }
    };

    let identity = LocalMacIdentity {
        owner_id: authenticated.owner_id,
        node_id: authenticated.node_id.clone(),
        computer_id: authenticated.computer_id,
        installation_id: authenticated.installation_id,
    };
    let (outbound_tx, mut outbound_rx) = mpsc::channel(32);
    let registered = state
        .local_mac_sessions
        .register(identity.clone(), outbound_tx);
    if let Err(error) = touch_node_connected(&state.pool, &authenticated.node_id).await {
        tracing::warn!(error = %error, "failed to record local Mac last_connected_at");
    }

    let ack = HostToMacMessage::HelloAck {
        protocol_version: PROTOCOL_VERSION,
        session_id: registered.session_id.clone(),
    };
    if send_json(&mut socket, &ack).await.is_err() {
        state.local_mac_sessions.unregister(
            &identity.owner_id,
            &identity.computer_id,
            registered.generation,
        );
        return;
    }

    tracing::info!(
        owner_id = %identity.owner_id,
        computer_id = %identity.computer_id,
        node_id = %identity.node_id,
        generation = registered.generation,
        "local Mac session established"
    );

    let (mut sink, mut stream) = socket.split();
    let mut ping = tokio::time::interval(PING_INTERVAL);
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    ping.tick().await;

    loop {
        tokio::select! {
            outbound = outbound_rx.recv() => {
                match outbound {
                    Some(message) => {
                        if send_json_sink(&mut sink, &message).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
            incoming = stream.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<MacToHostMessage>(text.as_str()) {
                            Ok(MacToHostMessage::Heartbeat { .. }) => {
                                if let Some(session) = state.local_mac_sessions.get(&identity.owner_id, &identity.computer_id) {
                                    if session.generation() == registered.generation {
                                        session.touch_heartbeat();
                                    }
                                }
                                let _ = send_json_sink(&mut sink, &HostToMacMessage::HeartbeatAck {
                                    protocol_version: PROTOCOL_VERSION,
                                }).await;
                            }
                            Ok(message @ MacToHostMessage::RpcResult { .. }) => {
                                if let Some(session) = state.local_mac_sessions.get(&identity.owner_id, &identity.computer_id) {
                                    if session.generation() == registered.generation {
                                        session.complete(message);
                                    }
                                }
                            }
                            Ok(MacToHostMessage::Hello { .. }) => {}
                            Err(_) => {
                                tracing::debug!("ignoring malformed local Mac session message");
                            }
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        if sink.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Pong(_))) => {
                        if let Some(session) = state.local_mac_sessions.get(&identity.owner_id, &identity.computer_id) {
                            if session.generation() == registered.generation {
                                session.touch_heartbeat();
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            _ = ping.tick() => {
                if sink.send(Message::Ping(Vec::new().into())).await.is_err() {
                    break;
                }
            }
        }

        if state
            .local_mac_sessions
            .generation_of(&identity.owner_id, &identity.computer_id)
            != Some(registered.generation)
        {
            break;
        }
    }

    state.local_mac_sessions.unregister(
        &identity.owner_id,
        &identity.computer_id,
        registered.generation,
    );
}

struct HelloClaim {
    node_id: String,
    computer_id: String,
}

async fn recv_hello(socket: &mut WebSocket) -> Result<HelloClaim, ()> {
    loop {
        match socket.recv().await {
            Some(Ok(Message::Text(text))) => {
                match serde_json::from_str::<MacToHostMessage>(text.as_str()) {
                    Ok(MacToHostMessage::Hello {
                        protocol_version,
                        node_id,
                        computer_id,
                    }) if protocol_version == PROTOCOL_VERSION => {
                        return Ok(HelloClaim {
                            node_id,
                            computer_id,
                        });
                    }
                    _ => return Err(()),
                }
            }
            Some(Ok(Message::Ping(payload))) => {
                let _ = socket.send(Message::Pong(payload)).await;
            }
            Some(Ok(Message::Pong(_))) => {}
            _ => return Err(()),
        }
    }
}

async fn send_json(socket: &mut WebSocket, message: &HostToMacMessage) -> Result<(), ()> {
    let json = serde_json::to_string(message).map_err(|_| ())?;
    socket.send(Message::text(json)).await.map_err(|_| ())
}

async fn send_json_sink(
    sink: &mut SplitSink<WebSocket, Message>,
    message: &HostToMacMessage,
) -> Result<(), ()> {
    let json = serde_json::to_string(message).map_err(|_| ())?;
    sink.send(Message::text(json)).await.map_err(|_| ())
}
