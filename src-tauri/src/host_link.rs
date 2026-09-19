//! Outbound authenticated WebSocket from the Tauri host to cloud-host.
//!
//! Default topology:
//! - debug: `ws://127.0.0.1:8080/v1/local-mac/sessions`
//! - release: `wss://elsewhere-alpha-runner.fly.dev/v1/local-mac/sessions`
//!
//! Override with `ELSEWHERE_CLOUD_HOST_WS_URL`.
//!
//! Pairing still goes through the www BFF. The live session is a direct
//! outbound WebSocket to cloud-host because Next.js does not proxy WS and
//! the runner remains internet-addressable on Fly HTTP.

use crate::agent::LocalMacComputer;
use crate::db::Database;
use crate::secrets::SecretStore;
use crate::vm::SharedVirtualMachineManager;
use agent_core::{HostToMacMessage, LocalMacRpcDispatcher, MacToHostMessage, PROTOCOL_VERSION};
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex as ParkingMutex;
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::AUTHORIZATION;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};

const STABLE_RESET_AFTER: Duration = Duration::from_secs(15);
const SESSION_PATH: &str = "/v1/local-mac/sessions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum HostLinkState {
    Disconnected,
    Connecting,
    Connected,
    ReauthRequired,
    Paused,
}

#[derive(Clone)]
pub struct HostLinkHandle {
    wakeup: Arc<Notify>,
    state: Arc<ParkingMutex<HostLinkState>>,
    reconnecting: Arc<ParkingMutex<bool>>,
    /// Wakes an active WebSocket session so pause / credential rotation can drop the link.
    session_break: Arc<Notify>,
}

impl HostLinkHandle {
    pub fn state(&self) -> HostLinkState {
        *self.state.lock()
    }

    pub fn notify_credential_ready(&self) {
        self.wakeup.notify_waiters();
        self.wakeup.notify_one();
    }

    pub fn reconnecting(&self) -> bool {
        *self.reconnecting.lock()
    }

    pub fn break_active_session(&self) {
        self.session_break.notify_waiters();
        self.session_break.notify_one();
    }
}

pub fn cloud_host_ws_url() -> String {
    if let Some(url) = std::env::var("ELSEWHERE_CLOUD_HOST_WS_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return url;
    }
    let origin = if cfg!(debug_assertions) {
        "ws://127.0.0.1:8080"
    } else {
        "wss://elsewhere-alpha-runner.fly.dev"
    };
    format!("{origin}{SESSION_PATH}")
}

pub fn start_host_link(
    db: Arc<ParkingMutex<Database>>,
    secrets: Arc<dyn SecretStore>,
    vm: SharedVirtualMachineManager,
) -> HostLinkHandle {
    let handle = HostLinkHandle {
        wakeup: Arc::new(Notify::new()),
        state: Arc::new(ParkingMutex::new(HostLinkState::Disconnected)),
        reconnecting: Arc::new(ParkingMutex::new(false)),
        session_break: Arc::new(Notify::new()),
    };
    let supervisor = handle.clone();
    tauri::async_runtime::spawn(async move {
        run_supervisor(supervisor, db, secrets, vm).await;
    });
    handle
}

async fn run_supervisor(
    handle: HostLinkHandle,
    db: Arc<ParkingMutex<Database>>,
    secrets: Arc<dyn SecretStore>,
    vm: SharedVirtualMachineManager,
) {
    let dispatcher = Arc::new(LocalMacRpcDispatcher::new(Arc::new(LocalMacComputer::new(
        vm,
    ))));
    let mut attempt: u32 = 0;
    loop {
        if this_mac_paused(&db) {
            set_state(&handle, HostLinkState::Paused);
            *handle.reconnecting.lock() = false;
            handle.wakeup.notified().await;
            continue;
        }
        let identity = {
            let db = db.lock();
            match db.elsewhere_pairing_identity() {
                Ok(value) => value,
                Err(error) => {
                    tracing::warn!(error = %error, "host link could not read pairing identity");
                    None
                }
            }
        };
        let credential = match secrets.get_elsewhere_device_credential() {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(error = %error, "host link could not read device credential");
                None
            }
        };
        let Some(((node_id, computer_id), credential)) = identity.zip(credential) else {
            set_state(&handle, HostLinkState::Disconnected);
            handle.wakeup.notified().await;
            continue;
        };

        set_state(&handle, HostLinkState::Connecting);
        *handle.reconnecting.lock() = attempt > 0;
        match connect_and_serve(
            &handle,
            dispatcher.clone(),
            &credential,
            &node_id,
            &computer_id,
        )
        .await
        {
            Ok(()) => {
                attempt = 0;
                *handle.reconnecting.lock() = false;
                set_state(&handle, HostLinkState::Disconnected);
            }
            Err(LinkError::AuthRejected) => {
                *handle.reconnecting.lock() = false;
                set_state(&handle, HostLinkState::ReauthRequired);
                tracing::warn!("local Mac device credential was rejected; waiting to re-pair");
                handle.wakeup.notified().await;
                attempt = 0;
            }
            Err(LinkError::Transport(reason)) => {
                set_state(&handle, HostLinkState::Disconnected);
                attempt = attempt.saturating_add(1);
                *handle.reconnecting.lock() = attempt > 0;
                let delay = backoff_delay(attempt);
                tracing::info!(error = %reason, ?delay, "local Mac host link reconnecting");
                tokio::select! {
                    _ = tokio::time::sleep(delay) => {}
                    _ = handle.wakeup.notified() => {}
                }
            }
        }
    }
}

#[derive(Debug)]
enum LinkError {
    AuthRejected,
    Transport(String),
}

async fn connect_and_serve(
    handle: &HostLinkHandle,
    dispatcher: Arc<LocalMacRpcDispatcher>,
    credential: &str,
    node_id: &str,
    computer_id: &str,
) -> Result<(), LinkError> {
    let url = cloud_host_ws_url();
    let mut request = url
        .into_client_request()
        .map_err(|e| LinkError::Transport(e.to_string()))?;
    let auth = format!("Bearer {credential}");
    request.headers_mut().insert(
        AUTHORIZATION,
        auth.parse().map_err(
            |e: tokio_tungstenite::tungstenite::http::header::InvalidHeaderValue| {
                LinkError::Transport(e.to_string())
            },
        )?,
    );
    let (ws, _) = match tokio_tungstenite::connect_async(request).await {
        Ok(pair) => pair,
        Err(WsError::Http(response)) if response.status() == 401 => {
            return Err(LinkError::AuthRejected);
        }
        Err(error) => return Err(LinkError::Transport(error.to_string())),
    };

    let (mut sink, mut stream) = ws.split();
    let hello = MacToHostMessage::Hello {
        protocol_version: PROTOCOL_VERSION,
        node_id: node_id.to_string(),
        computer_id: computer_id.to_string(),
    };
    let hello_json =
        serde_json::to_string(&hello).map_err(|e| LinkError::Transport(e.to_string()))?;
    sink.send(Message::Text(hello_json.into()))
        .await
        .map_err(|e| LinkError::Transport(e.to_string()))?;

    let first = tokio::time::timeout(Duration::from_secs(10), stream.next())
        .await
        .map_err(|_| LinkError::Transport("hello ack timed out".into()))?;
    match first {
        Some(Ok(Message::Text(text))) => match serde_json::from_str::<HostToMacMessage>(&text) {
            Ok(HostToMacMessage::HelloAck { .. }) => {}
            Ok(_) => return Err(LinkError::Transport("expected hello_ack".into())),
            Err(_) => return Err(LinkError::Transport("malformed hello_ack".into())),
        },
        Some(Ok(Message::Close(frame))) => {
            if frame
                .as_ref()
                .is_some_and(|close| close.code == tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Policy)
            {
                return Err(LinkError::AuthRejected);
            }
            return Err(LinkError::Transport("socket closed during hello".into()));
        }
        Some(Err(error)) => return Err(LinkError::Transport(error.to_string())),
        _ => return Err(LinkError::Transport("socket closed during hello".into())),
    }

    set_state(handle, HostLinkState::Connected);
    let connected_at = tokio::time::Instant::now();

    loop {
        tokio::select! {
            _ = handle.session_break.notified() => {
                let _ = sink.send(Message::Close(None)).await;
                set_state(handle, HostLinkState::Disconnected);
                return Ok(());
            }
            incoming = stream.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<HostToMacMessage>(&text) {
                            Ok(HostToMacMessage::Rpc {
                                id,
                                protocol_version,
                                method,
                                params,
                                deadline_ms: _,
                            }) => {
                                let result = dispatcher
                                    .handle_rpc(id, protocol_version, method, params)
                                    .await;
                                let json = serde_json::to_string(&result)
                                    .map_err(|e| LinkError::Transport(e.to_string()))?;
                                sink.send(Message::Text(json.into()))
                                    .await
                                    .map_err(|e| LinkError::Transport(e.to_string()))?;
                            }
                            Ok(HostToMacMessage::HeartbeatAck { .. })
                            | Ok(HostToMacMessage::HelloAck { .. }) => {}
                            Err(_) => {
                                tracing::debug!("ignoring malformed host rpc");
                            }
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        sink.send(Message::Pong(payload))
                            .await
                            .map_err(|e| LinkError::Transport(e.to_string()))?;
                    }
                    Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(Message::Close(frame))) => {
                        if connected_at.elapsed() >= STABLE_RESET_AFTER {
                            return Ok(());
                        }
                        if frame.as_ref().is_some_and(|close| {
                            close.code
                                == tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Policy
                        }) {
                            return Err(LinkError::AuthRejected);
                        }
                        return Err(LinkError::Transport("socket closed".into()));
                    }
                    Some(Ok(_)) => {}
                    Some(Err(error)) => {
                        if connected_at.elapsed() >= STABLE_RESET_AFTER {
                            return Ok(());
                        }
                        return Err(LinkError::Transport(error.to_string()));
                    }
                    None => {
                        if connected_at.elapsed() >= STABLE_RESET_AFTER {
                            return Ok(());
                        }
                        return Err(LinkError::Transport("socket closed".into()));
                    }
                }
            }
        }
    }
}

fn set_state(handle: &HostLinkHandle, state: HostLinkState) {
    *handle.state.lock() = state;
}

fn this_mac_paused(db: &Arc<ParkingMutex<Database>>) -> bool {
    let db = db.lock();
    db.this_mac_paused().unwrap_or(false)
}

pub fn backoff_delay(attempt: u32) -> Duration {
    let shift = attempt.min(6);
    let secs = (1u64 << shift).min(60);
    let jitter_ms = (uuid::Uuid::new_v4().as_u128() % 400) as u64;
    Duration::from_millis(secs * 1000 + jitter_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_is_bounded() {
        let delay = backoff_delay(20);
        assert!(delay <= Duration::from_millis(60_400));
        assert!(backoff_delay(0) >= Duration::from_secs(1));
    }

    #[test]
    fn default_ws_url_has_session_path() {
        std::env::remove_var("ELSEWHERE_CLOUD_HOST_WS_URL");
        assert!(cloud_host_ws_url().ends_with("/v1/local-mac/sessions"));
    }
}
