//! In-memory registry of authenticated outbound Mac sessions.
//!
//! Topology: Mac opens TLS WebSocket to cloud-host `/v1/local-mac/sessions`,
//! authenticates the `emac_` device credential, and stays registered until
//! disconnect. Newest successfully authenticated connection replaces the
//! previous session for the same `(owner_id, computer_id)`. The old connection
//! stops receiving RPCs. Mutations that already left the host are reported as
//! `ComputerError::AmbiguousOutcome` and are never automatically resent.

use agent_core::{
    computer_error_from_rpc, computer_info_from_result, encode_bytes, exec_result_from_result,
    is_mutation_method, read_file_bytes_from_result, workspace_entries_from_result, AgentComputer,
    ComputerError, ComputerInfo, ExecParams, ExecResult, HostToMacMessage, LocalMacRpcDispatcher,
    MacToHostMessage, PathParams, WorkspaceEntry, WriteFileParams, AMBIGUOUS_MUTATION_MESSAGE,
    CONTENT_ENCODING_BASE64, LOCAL_MAC_NOT_CONNECTED, METHOD_ENSURE_READY, METHOD_EXEC,
    METHOD_LIST_DIR, METHOD_READ_FILE, METHOD_WRITE_FILE,
};
use dashmap::DashMap;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

pub const MAX_PENDING_RPCS: usize = 32;
pub const DEFAULT_RPC_DEADLINE: Duration = Duration::from_secs(60);
pub const ENSURE_READY_DEADLINE: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalMacIdentity {
    pub owner_id: String,
    pub node_id: String,
    pub computer_id: String,
    pub installation_id: String,
}

impl LocalMacIdentity {
    pub fn key(&self) -> (String, String) {
        (self.owner_id.clone(), self.computer_id.clone())
    }
}

#[derive(Debug)]
struct PendingRpc {
    tx: oneshot::Sender<Result<Value, ComputerError>>,
    mutation: bool,
    dispatched: bool,
}

struct SessionState {
    identity: LocalMacIdentity,
    generation: u64,
    outbound: mpsc::Sender<HostToMacMessage>,
    pending: Mutex<HashMap<String, PendingRpc>>,
    last_heartbeat: Mutex<Instant>,
    replaced: AtomicBool,
}

#[derive(Clone, Default)]
pub struct LocalMacSessionRegistry {
    sessions: Arc<DashMap<(String, String), Arc<LocalMacSession>>>,
}

#[derive(Clone)]
pub struct LocalMacSession {
    inner: Arc<SessionState>,
}

#[derive(Clone)]
pub struct RegisteredLocalMacSession {
    pub identity: LocalMacIdentity,
    pub generation: u64,
    pub session_id: String,
}

impl LocalMacSessionRegistry {
    pub fn is_connected(&self, owner_id: &str, computer_id: &str) -> bool {
        self.sessions
            .contains_key(&(owner_id.to_string(), computer_id.to_string()))
    }

    pub fn get(&self, owner_id: &str, computer_id: &str) -> Option<Arc<LocalMacSession>> {
        self.sessions
            .get(&(owner_id.to_string(), computer_id.to_string()))
            .map(|entry| entry.value().clone())
    }

    /// Register an authenticated session. The newest connection wins.
    pub fn register(
        &self,
        identity: LocalMacIdentity,
        outbound: mpsc::Sender<HostToMacMessage>,
    ) -> RegisteredLocalMacSession {
        let key = identity.key();
        let generation = self
            .sessions
            .get(&key)
            .map(|existing| existing.inner.generation.saturating_add(1))
            .unwrap_or(1);
        if let Some((_, previous)) = self.sessions.remove(&key) {
            previous.mark_replaced();
            previous.fail_all(ComputerError::GuestUnavailable(
                "local Mac session replaced by a newer connection".into(),
            ));
        }
        let session = Arc::new(LocalMacSession {
            inner: Arc::new(SessionState {
                identity: identity.clone(),
                generation,
                outbound,
                pending: Mutex::new(HashMap::new()),
                last_heartbeat: Mutex::new(Instant::now()),
                replaced: AtomicBool::new(false),
            }),
        });
        self.sessions.insert(key, session);
        RegisteredLocalMacSession {
            identity,
            generation,
            session_id: Uuid::new_v4().to_string(),
        }
    }

    pub fn unregister(&self, owner_id: &str, computer_id: &str, generation: u64) {
        let key = (owner_id.to_string(), computer_id.to_string());
        let should_remove = self
            .sessions
            .get(&key)
            .map(|entry| entry.inner.generation == generation)
            .unwrap_or(false);
        if should_remove {
            if let Some((_, session)) = self.sessions.remove(&key) {
                session.fail_all(ComputerError::GuestUnavailable(
                    LOCAL_MAC_NOT_CONNECTED.into(),
                ));
            }
        }
    }

    pub fn generation_of(&self, owner_id: &str, computer_id: &str) -> Option<u64> {
        self.sessions
            .get(&(owner_id.to_string(), computer_id.to_string()))
            .map(|entry| entry.inner.generation)
    }

    pub fn pending_count(&self, owner_id: &str, computer_id: &str) -> usize {
        self.sessions
            .get(&(owner_id.to_string(), computer_id.to_string()))
            .map(|entry| {
                entry
                    .inner
                    .pending
                    .lock()
                    .map(|guard| guard.len())
                    .unwrap_or(0)
            })
            .unwrap_or(0)
    }
}

impl LocalMacSession {
    pub fn identity(&self) -> &LocalMacIdentity {
        &self.inner.identity
    }

    pub fn generation(&self) -> u64 {
        self.inner.generation
    }

    pub fn is_replaced(&self) -> bool {
        self.inner.replaced.load(Ordering::SeqCst)
    }

    pub fn touch_heartbeat(&self) {
        *self.inner.last_heartbeat.lock().expect("heartbeat lock") = Instant::now();
    }

    pub fn last_heartbeat(&self) -> Instant {
        *self.inner.last_heartbeat.lock().expect("heartbeat lock")
    }

    fn mark_replaced(&self) {
        self.inner.replaced.store(true, Ordering::SeqCst);
    }

    fn fail_all(&self, error: ComputerError) {
        let mut pending = self.inner.pending.lock().expect("pending rpc lock");
        for (_, pending_rpc) in pending.drain() {
            let err = if pending_rpc.mutation && pending_rpc.dispatched {
                ComputerError::AmbiguousOutcome(AMBIGUOUS_MUTATION_MESSAGE.into())
            } else {
                error.clone()
            };
            let _ = pending_rpc.tx.send(Err(err));
        }
    }

    pub fn complete(&self, message: MacToHostMessage) {
        self.touch_heartbeat();
        let MacToHostMessage::RpcResult {
            id,
            ok,
            result,
            error,
            ..
        } = message
        else {
            return;
        };
        let pending_rpc = {
            let mut pending = self.inner.pending.lock().expect("pending rpc lock");
            pending.remove(&id)
        };
        let Some(pending_rpc) = pending_rpc else {
            return;
        };
        let mapped = if ok {
            Ok(result.unwrap_or_else(|| json!({})))
        } else {
            Err(error
                .map(|err| computer_error_from_rpc(&err))
                .unwrap_or_else(|| ComputerError::ExecutionFailed("rpc failed".into())))
        };
        let _ = pending_rpc.tx.send(mapped);
    }

    pub async fn rpc(
        &self,
        method: &str,
        params: Value,
        deadline: Duration,
    ) -> Result<Value, ComputerError> {
        if self.is_replaced() {
            return Err(ComputerError::GuestUnavailable(
                "local Mac session replaced by a newer connection".into(),
            ));
        }
        let mutation = is_mutation_method(method);
        let id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.inner.pending.lock().expect("pending rpc lock");
            if pending.len() >= MAX_PENDING_RPCS {
                return Err(ComputerError::ExecutionFailed(
                    "too many pending local Mac requests".into(),
                ));
            }
            pending.insert(
                id.clone(),
                PendingRpc {
                    tx,
                    mutation,
                    dispatched: false,
                },
            );
        }

        let message =
            HostToMacMessage::rpc(id.clone(), method, params, deadline.as_millis() as u64);
        let send_result = self.inner.outbound.send(message).await;
        {
            let mut pending = self.inner.pending.lock().expect("pending rpc lock");
            if let Some(entry) = pending.get_mut(&id) {
                entry.dispatched = send_result.is_ok();
            }
        }
        if send_result.is_err() {
            let mut pending = self.inner.pending.lock().expect("pending rpc lock");
            pending.remove(&id);
            return Err(ComputerError::GuestUnavailable(
                LOCAL_MAC_NOT_CONNECTED.into(),
            ));
        }

        match tokio::time::timeout(deadline, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => {
                let dispatched_mutation = take_dispatched_mutation(&self.inner.pending, &id);
                if dispatched_mutation {
                    Err(ComputerError::AmbiguousOutcome(
                        AMBIGUOUS_MUTATION_MESSAGE.into(),
                    ))
                } else {
                    Err(ComputerError::GuestUnavailable(
                        LOCAL_MAC_NOT_CONNECTED.into(),
                    ))
                }
            }
            Err(_) => {
                let dispatched_mutation = take_dispatched_mutation(&self.inner.pending, &id);
                if dispatched_mutation {
                    Err(ComputerError::AmbiguousOutcome(
                        AMBIGUOUS_MUTATION_MESSAGE.into(),
                    ))
                } else {
                    Err(ComputerError::GuestUnavailable(
                        "local Mac request timed out".into(),
                    ))
                }
            }
        }
    }
}

fn take_dispatched_mutation(pending: &Mutex<HashMap<String, PendingRpc>>, id: &str) -> bool {
    pending
        .lock()
        .expect("pending rpc lock")
        .remove(id)
        .map(|entry| entry.dispatched && entry.mutation)
        .unwrap_or(false)
}

/// Hosted `AgentComputer` that forwards workspace ops to a live Mac session.
pub struct RemoteLocalMacComputer {
    owner_id: String,
    computer_id: String,
    node_id: String,
    registry: LocalMacSessionRegistry,
}

impl RemoteLocalMacComputer {
    pub fn new(
        owner_id: impl Into<String>,
        computer_id: impl Into<String>,
        node_id: impl Into<String>,
        registry: LocalMacSessionRegistry,
    ) -> Self {
        Self {
            owner_id: owner_id.into(),
            computer_id: computer_id.into(),
            node_id: node_id.into(),
            registry,
        }
    }

    pub fn owner_id(&self) -> &str {
        &self.owner_id
    }

    pub fn computer_id(&self) -> &str {
        &self.computer_id
    }

    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    fn session(&self) -> Result<Arc<LocalMacSession>, ComputerError> {
        let session = self
            .registry
            .get(&self.owner_id, &self.computer_id)
            .ok_or_else(|| ComputerError::GuestUnavailable(LOCAL_MAC_NOT_CONNECTED.into()))?;
        if session.identity().node_id != self.node_id {
            return Err(ComputerError::GuestUnavailable(
                "local Mac session identity mismatch".into(),
            ));
        }
        Ok(session)
    }

    async fn call(
        &self,
        method: &str,
        params: Value,
        deadline: Duration,
    ) -> Result<Value, ComputerError> {
        self.session()?.rpc(method, params, deadline).await
    }
}

#[async_trait::async_trait]
impl AgentComputer for RemoteLocalMacComputer {
    async fn ensure_ready(&self) -> Result<ComputerInfo, ComputerError> {
        let value = self
            .call(METHOD_ENSURE_READY, json!({}), ENSURE_READY_DEADLINE)
            .await?;
        computer_info_from_result(&value)
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<WorkspaceEntry>, ComputerError> {
        let params = serde_json::to_value(PathParams {
            path: path.to_string(),
        })
        .map_err(|e| ComputerError::MalformedArguments(e.to_string()))?;
        let value = self
            .call(METHOD_LIST_DIR, params, DEFAULT_RPC_DEADLINE)
            .await?;
        workspace_entries_from_result(&value)
    }

    async fn read_file(&self, path: &str) -> Result<Vec<u8>, ComputerError> {
        let params = serde_json::to_value(PathParams {
            path: path.to_string(),
        })
        .map_err(|e| ComputerError::MalformedArguments(e.to_string()))?;
        let value = self
            .call(METHOD_READ_FILE, params, DEFAULT_RPC_DEADLINE)
            .await?;
        read_file_bytes_from_result(&value)
    }

    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), ComputerError> {
        let params = serde_json::to_value(WriteFileParams {
            path: path.to_string(),
            content: encode_bytes(data),
            encoding: CONTENT_ENCODING_BASE64.to_string(),
        })
        .map_err(|e| ComputerError::MalformedArguments(e.to_string()))?;
        self.call(METHOD_WRITE_FILE, params, DEFAULT_RPC_DEADLINE)
            .await?;
        Ok(())
    }

    async fn exec(&self, command: &str) -> Result<ExecResult, ComputerError> {
        let params = serde_json::to_value(ExecParams {
            command: command.to_string(),
        })
        .map_err(|e| ComputerError::MalformedArguments(e.to_string()))?;
        let value = self.call(METHOD_EXEC, params, DEFAULT_RPC_DEADLINE).await?;
        exec_result_from_result(&value)
    }
}

pub async fn run_workspace_rpc_proof(
    computer: &dyn AgentComputer,
) -> Result<String, ComputerError> {
    computer.ensure_ready().await?;
    let path = "/workspace/remote-local-mac-proof.txt";
    let expected = b"hello from RemoteLocalMacComputer";
    computer.write_file(path, expected).await?;
    let read = computer.read_file(path).await?;
    if read != expected {
        return Err(ComputerError::ExecutionFailed("proof read mismatch".into()));
    }
    let exec = computer
        .exec("cat /workspace/remote-local-mac-proof.txt")
        .await?;
    if !exec.ok || exec.stdout.trim() != "hello from RemoteLocalMacComputer" {
        return Err(ComputerError::ExecutionFailed(format!(
            "proof exec mismatch: ok={} stdout={:?} stderr={:?}",
            exec.ok, exec.stdout, exec.stderr
        )));
    }
    Ok(exec.stdout)
}

/// Drive a fake Mac over the in-process session channels (no WebSocket).
pub fn spawn_in_process_mac(
    registry: &LocalMacSessionRegistry,
    identity: LocalMacIdentity,
    _dispatcher: Arc<LocalMacRpcDispatcher>,
) -> (Arc<LocalMacSession>, mpsc::Receiver<HostToMacMessage>) {
    let (tx, rx) = mpsc::channel(32);
    registry.register(identity.clone(), tx);
    let session = registry
        .get(&identity.owner_id, &identity.computer_id)
        .expect("registered");
    (session, rx)
}

pub fn spawn_in_process_mac_loop(
    registry: LocalMacSessionRegistry,
    identity: LocalMacIdentity,
    dispatcher: Arc<LocalMacRpcDispatcher>,
) -> Arc<LocalMacSession> {
    let (session, mut rx) = spawn_in_process_mac(&registry, identity.clone(), dispatcher.clone());
    let worker = session.clone();
    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if let HostToMacMessage::Rpc {
                id,
                protocol_version,
                method,
                params,
                ..
            } = message
            {
                let result = dispatcher
                    .handle_rpc(id, protocol_version, method, params)
                    .await;
                worker.complete(result);
            }
        }
    });
    session
}

/// Mac loop that executes the RPC then drops the connection before sending a result.
pub fn spawn_in_process_mac_drop_after_dispatch(
    registry: LocalMacSessionRegistry,
    identity: LocalMacIdentity,
    dispatcher: Arc<LocalMacRpcDispatcher>,
) -> Arc<LocalMacSession> {
    let (session, mut rx) = spawn_in_process_mac(&registry, identity.clone(), dispatcher.clone());
    let owner_id = identity.owner_id.clone();
    let computer_id = identity.computer_id.clone();
    let generation = session.generation();
    tokio::spawn(async move {
        if let Some(HostToMacMessage::Rpc {
            id,
            protocol_version,
            method,
            params,
            ..
        }) = rx.recv().await
        {
            let _ = dispatcher
                .handle_rpc(id, protocol_version, method, params)
                .await;
            drop(rx);
            registry.unregister(&owner_id, &computer_id, generation);
        }
    });
    session
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::{FakeAgentComputer, METHOD_ENSURE_READY};
    use std::sync::Arc;
    use std::time::Duration;

    fn identity(suffix: &str) -> LocalMacIdentity {
        LocalMacIdentity {
            owner_id: format!("owner-{suffix}"),
            node_id: format!("node-{suffix}"),
            computer_id: format!("computer-{suffix}"),
            installation_id: format!("11111111-1111-1111-1111-{suffix:0<12}"),
        }
    }

    fn dispatcher() -> (Arc<FakeAgentComputer>, Arc<LocalMacRpcDispatcher>) {
        let fake = Arc::new(FakeAgentComputer::new().with_listing(
            "/workspace",
            vec![WorkspaceEntry {
                name: "a.txt".into(),
                path: "/workspace/a.txt".into(),
                is_dir: false,
            }],
        ));
        let dispatcher = Arc::new(LocalMacRpcDispatcher::new(fake.clone()));
        (fake, dispatcher)
    }

    #[tokio::test]
    async fn session_registers_and_disconnect_removes_it() {
        let registry = LocalMacSessionRegistry::default();
        let id = identity("a");
        let (_fake, disp) = dispatcher();
        let session = spawn_in_process_mac_loop(registry.clone(), id.clone(), disp);
        assert!(registry.is_connected(&id.owner_id, &id.computer_id));
        registry.unregister(&id.owner_id, &id.computer_id, session.generation());
        assert!(!registry.is_connected(&id.owner_id, &id.computer_id));
    }

    #[tokio::test]
    async fn newest_connection_replaces_previous() {
        let registry = LocalMacSessionRegistry::default();
        let id = identity("b");
        let (_fake, disp) = dispatcher();
        let first = spawn_in_process_mac_loop(registry.clone(), id.clone(), disp.clone());
        let first_generation = first.generation();
        let second = spawn_in_process_mac_loop(registry.clone(), id.clone(), disp);
        assert_ne!(second.generation(), first_generation);
        assert_eq!(
            registry.generation_of(&id.owner_id, &id.computer_id),
            Some(second.generation())
        );
        assert!(first.is_replaced());
        assert!(!second.is_replaced());
    }

    #[tokio::test]
    async fn remote_computer_roundtrip_offline_browser_and_binary() {
        let registry = LocalMacSessionRegistry::default();
        let id = identity("c");
        let (fake, disp) = dispatcher();
        spawn_in_process_mac_loop(registry.clone(), id.clone(), disp);
        let computer = RemoteLocalMacComputer::new(
            id.owner_id.clone(),
            id.computer_id.clone(),
            id.node_id.clone(),
            registry.clone(),
        );
        let info = computer.ensure_ready().await.unwrap();
        assert!(info.ready);
        let listing = computer.list_dir("/workspace").await.unwrap();
        assert_eq!(listing[0].name, "a.txt");
        computer
            .write_file(
                "/workspace/remote-local-mac-proof.txt",
                b"hello from RemoteLocalMacComputer",
            )
            .await
            .unwrap();
        assert_eq!(fake.write_file_calls(), 1);
        let proof = run_workspace_rpc_proof(&computer).await.unwrap();
        assert!(proof.contains("hello from RemoteLocalMacComputer"));
        assert_eq!(fake.write_file_calls(), 2);

        let binary = vec![0u8, 1, 255, 10, 0];
        computer
            .write_file("/workspace/bin.dat", &binary)
            .await
            .unwrap();
        assert_eq!(
            computer.read_file("/workspace/bin.dat").await.unwrap(),
            binary
        );

        let browser = computer
            .browser_invoke("navigate", &serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(matches!(browser, ComputerError::SandboxRejected(_)));

        registry.unregister(
            &id.owner_id,
            &id.computer_id,
            registry
                .generation_of(&id.owner_id, &id.computer_id)
                .unwrap(),
        );
        let err = computer.ensure_ready().await.unwrap_err();
        assert_eq!(
            err,
            ComputerError::GuestUnavailable(LOCAL_MAC_NOT_CONNECTED.into())
        );
    }

    #[tokio::test]
    async fn mutation_drop_is_ambiguous_and_not_resent() {
        let registry = LocalMacSessionRegistry::default();
        let drop_id = identity("e");
        let (drop_fake, drop_dispatcher) = dispatcher();
        spawn_in_process_mac_drop_after_dispatch(
            registry.clone(),
            drop_id.clone(),
            drop_dispatcher,
        );
        let drop_computer = RemoteLocalMacComputer::new(
            drop_id.owner_id.clone(),
            drop_id.computer_id.clone(),
            drop_id.node_id.clone(),
            registry.clone(),
        );
        let err = drop_computer
            .write_file("/workspace/maybe.txt", b"maybe")
            .await
            .unwrap_err();
        assert!(err.is_ambiguous(), "{err}");
        assert_eq!(drop_fake.write_file_calls(), 1);
        assert!(!registry.is_connected(&drop_id.owner_id, &drop_id.computer_id));
    }

    #[tokio::test]
    async fn wrong_request_id_does_not_complete_call() {
        let registry = LocalMacSessionRegistry::default();
        let id = identity("f");
        let (_fake, disp) = dispatcher();
        let (session, mut rx) = spawn_in_process_mac(&registry, id.clone(), disp);
        let worker = session.clone();
        tokio::spawn(async move {
            if let Some(HostToMacMessage::Rpc { .. }) = rx.recv().await {
                worker.complete(MacToHostMessage::rpc_ok(
                    "totally-different-id",
                    serde_json::json!({ "ready": true, "protocolVersion": 1 }),
                ));
            }
        });
        let computer =
            RemoteLocalMacComputer::new(id.owner_id, id.computer_id, id.node_id, registry);
        let result =
            tokio::time::timeout(Duration::from_millis(120), computer.ensure_ready()).await;
        assert!(
            result.is_err() || matches!(result, Ok(Err(ComputerError::GuestUnavailable(_)))),
            "wrong request id must not successfully complete ensure_ready: {result:?}"
        );
    }

    #[tokio::test]
    async fn malformed_response_is_safe() {
        let registry = LocalMacSessionRegistry::default();
        let id = identity("g");
        let (_fake, disp) = dispatcher();
        let (session, mut rx) = spawn_in_process_mac(&registry, id.clone(), disp);
        let worker = session.clone();
        tokio::spawn(async move {
            if let Some(HostToMacMessage::Rpc { id: rpc_id, .. }) = rx.recv().await {
                worker.complete(MacToHostMessage::rpc_ok(rpc_id, serde_json::json!(42)));
            }
        });
        let computer =
            RemoteLocalMacComputer::new(id.owner_id, id.computer_id, id.node_id, registry);
        let err = computer.ensure_ready().await.unwrap_err();
        assert!(matches!(err, ComputerError::ExecutionFailed(_)));
    }

    #[tokio::test]
    async fn timeout_clears_pending() {
        let registry = LocalMacSessionRegistry::default();
        let id = identity("h");
        let (tx, rx) = mpsc::channel(1);
        registry.register(id.clone(), tx);
        std::mem::forget(rx);
        let session = registry.get(&id.owner_id, &id.computer_id).unwrap();
        session
            .rpc(METHOD_ENSURE_READY, json!({}), Duration::from_millis(30))
            .await
            .unwrap_err();
        assert_eq!(registry.pending_count(&id.owner_id, &id.computer_id), 0);
    }

    #[tokio::test]
    async fn bounded_pending_rejects_without_dispatch() {
        let registry = LocalMacSessionRegistry::default();
        let id = identity("j");
        let (tx, rx) = mpsc::channel(MAX_PENDING_RPCS + 8);
        registry.register(id.clone(), tx);
        std::mem::forget(rx);
        let session = registry.get(&id.owner_id, &id.computer_id).unwrap();
        let mut joins = Vec::new();
        for _ in 0..MAX_PENDING_RPCS {
            let session = session.clone();
            joins.push(tokio::spawn(async move {
                session
                    .rpc(
                        METHOD_LIST_DIR,
                        json!({"path": "/workspace"}),
                        Duration::from_secs(5),
                    )
                    .await
            }));
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
        let err = session
            .rpc(
                METHOD_LIST_DIR,
                json!({"path": "/workspace"}),
                Duration::from_millis(20),
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("too many pending"), "{err}");
        for join in joins {
            let _ = join.await;
        }
    }

    #[tokio::test]
    async fn pending_rpc_fails_when_connection_drops() {
        let registry = LocalMacSessionRegistry::default();
        let id = identity("i");
        let (_fake, disp) = dispatcher();
        let (session, mut rx) = spawn_in_process_mac(&registry, id.clone(), disp);
        let generation = session.generation();
        let computer = RemoteLocalMacComputer::new(
            id.owner_id.clone(),
            id.computer_id.clone(),
            id.node_id.clone(),
            registry.clone(),
        );
        let pending = tokio::spawn(async move { computer.ensure_ready().await });
        let _ = rx.recv().await;
        drop(rx);
        registry.unregister(&id.owner_id, &id.computer_id, generation);
        let err = pending.await.unwrap().unwrap_err();
        assert!(matches!(err, ComputerError::GuestUnavailable(_)));
    }
}
