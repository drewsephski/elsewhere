//! Versioned local-Mac session protocol shared by cloud-host and the Tauri host.
//!
//! File bytes travel as base64 in JSON. The dispatcher executes only the v1
//! allowlist against an `AgentComputer` and caches completed results by request id.

use crate::computer::{AgentComputer, ComputerError, ComputerInfo, ExecResult, WorkspaceEntry};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use futures_util::future::{BoxFuture, Shared};
use futures_util::FutureExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub const PROTOCOL_VERSION: u32 = 1;
pub const CONTENT_ENCODING_BASE64: &str = "base64";
pub const METHOD_ENSURE_READY: &str = "ensure_ready";
pub const METHOD_LIST_DIR: &str = "list_dir";
pub const METHOD_READ_FILE: &str = "read_file";
pub const METHOD_WRITE_FILE: &str = "write_file";
pub const METHOD_EXEC: &str = "exec";
pub const LOCAL_MAC_NOT_CONNECTED: &str = "local Mac is not connected";
pub const AMBIGUOUS_MUTATION_MESSAGE: &str =
    "connection lost after dispatch; mutation may have already executed";

const DEDUPE_MAX: usize = 64;
const DEDUPE_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PathParams {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WriteFileParams {
    pub path: String,
    pub content: String,
    #[serde(default = "default_encoding")]
    pub encoding: String,
}

fn default_encoding() -> String {
    CONTENT_ENCODING_BASE64.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExecParams {
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReadFileResult {
    pub content: String,
    pub encoding: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MacToHostMessage {
    #[serde(rename_all = "camelCase")]
    Hello {
        protocol_version: u32,
        node_id: String,
        computer_id: String,
    },
    #[serde(rename_all = "camelCase")]
    RpcResult {
        protocol_version: u32,
        id: String,
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        result: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<RpcError>,
    },
    #[serde(rename_all = "camelCase")]
    Heartbeat { protocol_version: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostToMacMessage {
    #[serde(rename_all = "camelCase")]
    HelloAck {
        protocol_version: u32,
        session_id: String,
    },
    #[serde(rename_all = "camelCase")]
    Rpc {
        protocol_version: u32,
        id: String,
        method: String,
        params: Value,
        deadline_ms: u64,
    },
    #[serde(rename_all = "camelCase")]
    HeartbeatAck { protocol_version: u32 },
}

impl HostToMacMessage {
    pub fn rpc(
        id: impl Into<String>,
        method: impl Into<String>,
        params: Value,
        deadline_ms: u64,
    ) -> Self {
        Self::Rpc {
            protocol_version: PROTOCOL_VERSION,
            id: id.into(),
            method: method.into(),
            params,
            deadline_ms,
        }
    }
}

impl MacToHostMessage {
    pub fn rpc_ok(id: impl Into<String>, result: Value) -> Self {
        Self::RpcResult {
            protocol_version: PROTOCOL_VERSION,
            id: id.into(),
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn rpc_err(id: impl Into<String>, error: RpcError) -> Self {
        Self::RpcResult {
            protocol_version: PROTOCOL_VERSION,
            id: id.into(),
            ok: false,
            result: None,
            error: Some(error),
        }
    }
}

pub fn encode_bytes(data: &[u8]) -> String {
    BASE64.encode(data)
}

pub fn decode_bytes(encoded: &str) -> Result<Vec<u8>, String> {
    BASE64
        .decode(encoded.trim().as_bytes())
        .map_err(|e| format!("invalid base64 content: {e}"))
}

pub fn rpc_error_from_computer(error: &ComputerError) -> RpcError {
    RpcError {
        code: error.code().to_string(),
        message: error.to_string(),
    }
}

pub fn computer_error_from_rpc(error: &RpcError) -> ComputerError {
    match error.code.as_str() {
        "vm_not_provisioned" => ComputerError::NotProvisioned,
        "vm_boot_failed" => ComputerError::BootFailed(error.message.clone()),
        "guest_unavailable" => ComputerError::GuestUnavailable(error.message.clone()),
        "malformed_tool_arguments" => ComputerError::MalformedArguments(error.message.clone()),
        "tool_rejected_by_sandbox" => ComputerError::SandboxRejected(error.message.clone()),
        "computer_operation_ambiguous" => ComputerError::AmbiguousOutcome(error.message.clone()),
        "cancelled" => ComputerError::Cancelled,
        _ => ComputerError::ExecutionFailed(error.message.clone()),
    }
}

pub fn is_mutation_method(method: &str) -> bool {
    matches!(method, METHOD_WRITE_FILE | METHOD_EXEC)
}

type SharedRpc = Shared<BoxFuture<'static, MacToHostMessage>>;

/// Executes allowlisted v1 RPCs against an `AgentComputer`.
///
/// Duplicate request IDs reuse a cached result (or the in-flight future) and
/// do not re-execute. This is memory-local, not exactly-once across process restarts.
pub struct LocalMacRpcDispatcher {
    computer: Arc<dyn AgentComputer>,
    cache: Mutex<DedupeCache>,
}

struct DedupeCache {
    inflight: HashMap<String, SharedRpc>,
    done: HashMap<String, (Instant, MacToHostMessage)>,
    order: VecDeque<String>,
}

impl DedupeCache {
    fn prune(&mut self, now: Instant) {
        while let Some(id) = self.order.front().cloned() {
            let expired = self
                .done
                .get(&id)
                .map(|(at, _)| now.duration_since(*at) > DEDUPE_TTL)
                .unwrap_or(false);
            if expired || self.order.len() > DEDUPE_MAX {
                self.order.pop_front();
                self.done.remove(&id);
                self.inflight.remove(&id);
            } else {
                break;
            }
        }
    }
}

impl LocalMacRpcDispatcher {
    pub fn new(computer: Arc<dyn AgentComputer>) -> Self {
        Self {
            computer,
            cache: Mutex::new(DedupeCache {
                inflight: HashMap::new(),
                done: HashMap::new(),
                order: VecDeque::new(),
            }),
        }
    }

    pub async fn handle_rpc(
        &self,
        id: String,
        protocol_version: u32,
        method: String,
        params: Value,
    ) -> MacToHostMessage {
        if protocol_version != PROTOCOL_VERSION {
            return MacToHostMessage::rpc_err(
                id,
                RpcError {
                    code: "protocol_version_unsupported".into(),
                    message: format!("unsupported protocol version {protocol_version}"),
                },
            );
        }

        let now = Instant::now();
        let cached = {
            let mut cache = self.cache.lock().await;
            cache.prune(now);
            if let Some((_, cached)) = cache.done.get(&id) {
                return cached.clone();
            }
            if let Some(existing) = cache.inflight.get(&id) {
                Some(existing.clone())
            } else {
                let computer = self.computer.clone();
                let method_cloned = method.clone();
                let params_cloned = params.clone();
                let id_cloned = id.clone();
                let fut = async move {
                    dispatch_once(computer.as_ref(), id_cloned, method_cloned, params_cloned).await
                }
                .boxed()
                .shared();
                cache.inflight.insert(id.clone(), fut.clone());
                Some(fut)
            }
        };
        let result = cached.expect("inflight future").await;
        let mut cache = self.cache.lock().await;
        cache.inflight.remove(&id);
        cache
            .done
            .insert(id.clone(), (Instant::now(), result.clone()));
        cache.order.push_back(id);
        cache.prune(Instant::now());
        result
    }
}

async fn dispatch_once(
    computer: &dyn AgentComputer,
    id: String,
    method: String,
    params: Value,
) -> MacToHostMessage {
    match execute_method(computer, &method, params).await {
        Ok(result) => MacToHostMessage::rpc_ok(id, result),
        Err(error) => MacToHostMessage::rpc_err(id, rpc_error_from_computer(&error)),
    }
}

async fn execute_method(
    computer: &dyn AgentComputer,
    method: &str,
    params: Value,
) -> Result<Value, ComputerError> {
    match method {
        METHOD_ENSURE_READY => {
            let info = computer.ensure_ready().await?;
            serde_json::to_value(info).map_err(|e| ComputerError::ExecutionFailed(e.to_string()))
        }
        METHOD_LIST_DIR => {
            let parsed: PathParams = serde_json::from_value(params)
                .map_err(|e| ComputerError::MalformedArguments(format!("list_dir params: {e}")))?;
            let entries = computer.list_dir(&parsed.path).await?;
            serde_json::to_value(entries).map_err(|e| ComputerError::ExecutionFailed(e.to_string()))
        }
        METHOD_READ_FILE => {
            let parsed: PathParams = serde_json::from_value(params)
                .map_err(|e| ComputerError::MalformedArguments(format!("read_file params: {e}")))?;
            let bytes = computer.read_file(&parsed.path).await?;
            serde_json::to_value(ReadFileResult {
                content: encode_bytes(&bytes),
                encoding: CONTENT_ENCODING_BASE64.to_string(),
            })
            .map_err(|e| ComputerError::ExecutionFailed(e.to_string()))
        }
        METHOD_WRITE_FILE => {
            let parsed: WriteFileParams = serde_json::from_value(params).map_err(|e| {
                ComputerError::MalformedArguments(format!("write_file params: {e}"))
            })?;
            let bytes = decode_write_params(&parsed)?;
            computer.write_file(&parsed.path, &bytes).await?;
            Ok(json!({ "ok": true }))
        }
        METHOD_EXEC => {
            let parsed: ExecParams = serde_json::from_value(params)
                .map_err(|e| ComputerError::MalformedArguments(format!("exec params: {e}")))?;
            let result = computer.exec(&parsed.command).await?;
            serde_json::to_value(result).map_err(|e| ComputerError::ExecutionFailed(e.to_string()))
        }
        other => Err(ComputerError::MalformedArguments(format!(
            "unsupported method: {other}"
        ))),
    }
}

fn decode_write_params(params: &WriteFileParams) -> Result<Vec<u8>, ComputerError> {
    if params.encoding == CONTENT_ENCODING_BASE64 {
        decode_bytes(&params.content).map_err(ComputerError::MalformedArguments)
    } else {
        Ok(params.content.as_bytes().to_vec())
    }
}

pub fn read_file_bytes_from_result(value: &Value) -> Result<Vec<u8>, ComputerError> {
    let parsed: ReadFileResult = serde_json::from_value(value.clone())
        .map_err(|e| ComputerError::ExecutionFailed(format!("malformed read_file result: {e}")))?;
    if parsed.encoding == CONTENT_ENCODING_BASE64 {
        decode_bytes(&parsed.content).map_err(ComputerError::ExecutionFailed)
    } else {
        Ok(parsed.content.into_bytes())
    }
}

pub fn computer_info_from_result(value: &Value) -> Result<ComputerInfo, ComputerError> {
    serde_json::from_value(value.clone())
        .map_err(|e| ComputerError::ExecutionFailed(format!("malformed ensure_ready result: {e}")))
}

pub fn workspace_entries_from_result(value: &Value) -> Result<Vec<WorkspaceEntry>, ComputerError> {
    serde_json::from_value(value.clone())
        .map_err(|e| ComputerError::ExecutionFailed(format!("malformed list_dir result: {e}")))
}

pub fn exec_result_from_result(value: &Value) -> Result<ExecResult, ComputerError> {
    serde_json::from_value(value.clone())
        .map_err(|e| ComputerError::ExecutionFailed(format!("malformed exec result: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake_computer::FakeAgentComputer;
    use crate::WorkspaceEntry;

    #[test]
    fn hello_json_matches_v1_shape() {
        let message = MacToHostMessage::Hello {
            protocol_version: 1,
            node_id: "node-1".into(),
            computer_id: "comp-1".into(),
        };
        let value = serde_json::to_value(&message).unwrap();
        assert_eq!(value["type"], "hello");
        assert_eq!(value["protocolVersion"], 1);
        assert_eq!(value["nodeId"], "node-1");
        assert_eq!(value["computerId"], "comp-1");
    }

    #[test]
    fn rpc_json_matches_v1_shape() {
        let message = HostToMacMessage::rpc(
            "11111111-1111-1111-1111-111111111111",
            METHOD_READ_FILE,
            json!({ "path": "/workspace/foo.txt" }),
            60_000,
        );
        let value = serde_json::to_value(&message).unwrap();
        assert_eq!(value["type"], "rpc");
        assert_eq!(value["protocolVersion"], 1);
        assert_eq!(value["method"], "read_file");
        assert_eq!(value["deadlineMs"], 60_000);
    }

    #[test]
    fn binary_roundtrip_helpers() {
        let original = vec![0u8, 1, 2, 255, 0, 10, 128];
        let encoded = encode_bytes(&original);
        assert_eq!(decode_bytes(&encoded).unwrap(), original);
    }

    #[tokio::test]
    async fn dispatcher_executes_allowlist_and_dedupes_write() {
        let computer = Arc::new(FakeAgentComputer::new());
        let dispatcher = LocalMacRpcDispatcher::new(computer.clone());
        let params = serde_json::to_value(WriteFileParams {
            path: "/workspace/proof.bin".into(),
            content: encode_bytes(&[0, 255, 10]),
            encoding: CONTENT_ENCODING_BASE64.into(),
        })
        .unwrap();
        let first = dispatcher
            .handle_rpc("req-1".into(), 1, METHOD_WRITE_FILE.into(), params.clone())
            .await;
        let second = dispatcher
            .handle_rpc("req-1".into(), 1, METHOD_WRITE_FILE.into(), params)
            .await;
        assert_eq!(first, second);
        assert_eq!(computer.write_file_calls(), 1);
        assert_eq!(
            computer.file_contents("/workspace/proof.bin").unwrap(),
            vec![0, 255, 10]
        );
    }

    #[tokio::test]
    async fn dispatcher_rejects_unknown_method() {
        let dispatcher = LocalMacRpcDispatcher::new(Arc::new(FakeAgentComputer::new()));
        let MacToHostMessage::RpcResult { ok, error, .. } = dispatcher
            .handle_rpc("x".into(), 1, "rm_rf".into(), json!({}))
            .await
        else {
            panic!("expected rpc_result");
        };
        assert!(!ok);
        assert_eq!(error.unwrap().code, "malformed_tool_arguments");
    }

    #[tokio::test]
    async fn dispatcher_list_dir_roundtrip() {
        let computer = FakeAgentComputer::new().with_listing(
            "/workspace",
            vec![WorkspaceEntry {
                name: "a.txt".into(),
                path: "/workspace/a.txt".into(),
                is_dir: false,
            }],
        );
        let dispatcher = LocalMacRpcDispatcher::new(Arc::new(computer));
        let MacToHostMessage::RpcResult {
            ok,
            result: Some(result),
            ..
        } = dispatcher
            .handle_rpc(
                "list".into(),
                1,
                METHOD_LIST_DIR.into(),
                json!({ "path": "/workspace" }),
            )
            .await
        else {
            panic!("expected ok rpc_result");
        };
        assert!(ok);
        let entries = workspace_entries_from_result(&result).unwrap();
        assert_eq!(entries[0].name, "a.txt");
    }
}
