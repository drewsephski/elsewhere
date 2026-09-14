//! Scripted agent-core loop against SpriteComputer (mock API by default).
//!
//! Live run (optional):
//! `SPRITES_TOKEN=... ELSEWHERE_TEST_SPRITE=... cargo run -p sprite-computer --example agent_cloud_proof --features live`
//!
//! Mock run (CI-safe):
//! `cargo run -p sprite-computer --example agent_cloud_proof`

use agent_core::{
    run_agent_loop, AgentComputer, AgentEvent, AgentLoopContext, AgentLoopDeps, ComputerError,
    ComputerInfo, CreateResponseResult, CreateRunParams, EventSink, ExecResult, MessageStatus,
    PersistedMessage, ResponsesModel, RunStore, RuntimeError, StructuredMessageInput,
    WorkspaceEntry,
};
use async_trait::async_trait;
use serde_json::{json, Value};
use sprite_computer::{
    default_deny_network_policy, SpriteComputer, SpriteComputerConfig,
};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("SPRITES_TOKEN").is_ok() && std::env::var("ELSEWHERE_TEST_SPRITE").is_ok() {
        run_live().await?;
    } else {
        run_mock().await?;
    }
    Ok(())
}

async fn run_mock() -> Result<(), Box<dyn std::error::Error>> {
    let computer = Arc::new(InMemoryCloudComputer::default());
    run_scripted_agent(computer).await?;
    println!("agent-core + in-memory cloud computer proof passed");
    Ok(())
}

async fn run_live() -> Result<(), Box<dyn std::error::Error>> {
    let config = SpriteComputerConfig {
        base_url: std::env::var("SPRITES_API_BASE")
            .unwrap_or_else(|_| sprite_computer::DEFAULT_API_BASE.into()),
        token: std::env::var("SPRITES_TOKEN")?,
        sprite_name: std::env::var("ELSEWHERE_TEST_SPRITE")?,
        workspace_root: "/workspace".into(),
        request_timeout: Duration::from_secs(120),
        auto_create: true,
        network_policy: default_deny_network_policy(),
        exec_timeout: Duration::from_secs(60),
    };
    let computer = Arc::new(SpriteComputer::new(config)?);
    run_scripted_agent(computer).await?;
    println!("agent-core + SpriteComputer live proof passed");
    Ok(())
}

async fn run_scripted_agent(
    computer: Arc<dyn AgentComputer>,
) -> Result<(), Box<dyn std::error::Error>> {
    let model = Arc::new(ScriptedModel {
        steps: Mutex::new(vec![
            CreateResponseResult {
                output: vec![json!({
                    "type": "function_call",
                    "name": "workspace_write",
                    "call_id": "w1",
                    "arguments": "{\"path\":\"/workspace/from-agent.txt\",\"content\":\"cloud works\"}"
                })],
                output_text: None,
            },
            CreateResponseResult {
                output: vec![json!({
                    "type": "function_call",
                    "name": "workspace_read",
                    "call_id": "r1",
                    "arguments": "{\"path\":\"/workspace/from-agent.txt\"}"
                })],
                output_text: None,
            },
            CreateResponseResult {
                output: vec![json!({
                    "type": "message",
                    "content": [{"type": "output_text", "text": "done"}]
                })],
                output_text: Some("done".into()),
            },
        ]),
    });

    let deps = AgentLoopDeps {
        computer,
        store: Arc::new(MemStore::default()),
        events: Arc::new(RecordingEvents::default()),
        model,
        cancel: Arc::new(AtomicBool::new(false)),
    };

    let ctx = AgentLoopContext {
        request_id: "req-cloud".into(),
        conversation_id: "conv-cloud".into(),
        assistant_message_id: "asst-cloud".into(),
        bot_id: "bot-cloud".into(),
        model: agent_core::DEFAULT_MODEL.into(),
        instructions: String::new(),
    };

    run_agent_loop(ctx, deps, vec![json!({"role":"user","content":"prove cloud"})]).await?;
    Ok(())
}

#[derive(Default)]
struct InMemoryCloudComputer {
    files: Mutex<HashMap<String, Vec<u8>>>,
}

#[async_trait]
impl AgentComputer for InMemoryCloudComputer {
    async fn ensure_ready(&self) -> Result<ComputerInfo, ComputerError> {
        Ok(ComputerInfo {
            ready: true,
            protocol_version: 1,
            detail: Some("mock".into()),
        })
    }

    async fn list_dir(&self, _path: &str) -> Result<Vec<WorkspaceEntry>, ComputerError> {
        Ok(vec![])
    }

    async fn read_file(&self, path: &str) -> Result<Vec<u8>, ComputerError> {
        self.files
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| ComputerError::ExecutionFailed("not found".into()))
    }

    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), ComputerError> {
        self.files
            .lock()
            .unwrap()
            .insert(path.to_string(), data.to_vec());
        Ok(())
    }

    async fn exec(&self, _command: &str) -> Result<ExecResult, ComputerError> {
        Ok(ExecResult {
            ok: true,
            stdout: String::new(),
            stderr: String::new(),
            exit_code: 0,
        })
    }
}

#[derive(Default)]
struct MemStore {
    runs: Mutex<HashMap<String, String>>,
}

impl RunStore for MemStore {
    fn create_run(&self, params: CreateRunParams) -> Result<String, RuntimeError> {
        self.runs
            .lock()
            .unwrap()
            .insert(params.request_id.clone(), "running".into());
        Ok("run-1".into())
    }

    fn append_run_event(
        &self,
        _request_id: &str,
        _event_type: &str,
        _payload: &Value,
    ) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn persist_structured_message(
        &self,
        input: StructuredMessageInput,
    ) -> Result<PersistedMessage, RuntimeError> {
        Ok(PersistedMessage {
            id: "msg-1".into(),
            conversation_id: input.conversation_id,
            kind: input.kind,
            body: input.body,
        })
    }

    fn update_assistant_message(
        &self,
        _message_id: &str,
        _body: &str,
        _status: MessageStatus,
        _error_message: Option<&str>,
    ) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn update_run(
        &self,
        _request_id: &str,
        _status: &str,
        _error_code: Option<&str>,
        _step_count: i64,
    ) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn touch_conversation_and_bot(
        &self,
        _conversation_id: &str,
        _bot_id: &str,
    ) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn get_assistant_message_body(&self, _message_id: &str) -> Result<String, RuntimeError> {
        Ok(String::new())
    }
}

#[derive(Default)]
struct RecordingEvents {
    labels: Mutex<Vec<String>>,
}

impl EventSink for RecordingEvents {
    fn emit(&self, event: AgentEvent) -> Result<(), RuntimeError> {
        let label = match event {
            AgentEvent::ToolCall { .. } => "tool_call",
            AgentEvent::ToolResult { .. } => "tool_result",
            AgentEvent::RunCompleted { .. } => "completed",
            _ => "other",
        };
        self.labels.lock().unwrap().push(label.to_string());
        Ok(())
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
    ) -> Result<CreateResponseResult, agent_core::ModelError> {
        let mut steps = self.steps.lock().unwrap();
        if steps.is_empty() {
            return Ok(CreateResponseResult {
                output: vec![json!({
                    "type": "message",
                    "content": [{"type": "output_text", "text": "done"}]
                })],
                output_text: Some("done".into()),
            });
        }
        Ok(steps.remove(0))
    }
}
