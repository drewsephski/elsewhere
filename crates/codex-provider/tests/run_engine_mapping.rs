#![cfg(feature = "test-utils")]

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use agent_core::{
    AgentEvent, AgentLoopContext, EventSink, FakeAgentComputer, RunStore, RuntimeError,
    StructuredMessageInput,
};
use async_trait::async_trait;
use codex_provider::{
    spawn_fake_app_server_with_mode, CodexRunEngine, CodexRunEngineConfig, FakeServerMode,
};
use serde_json::json;

struct RecordingEvents {
    events: Mutex<Vec<String>>,
}

impl RecordingEvents {
    fn labels(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }
}

impl EventSink for RecordingEvents {
    fn emit(&self, event: AgentEvent) -> Result<(), RuntimeError> {
        let label = match event {
            AgentEvent::ToolCall { .. } => "tool_call",
            AgentEvent::ToolResult { .. } => "tool_result",
            AgentEvent::RunCompleted { .. } => "completed",
            AgentEvent::RunFailed { .. } => "failed",
            AgentEvent::RunCancelled => "cancelled",
            _ => "other",
        };
        self.events.lock().unwrap().push(label.to_string());
        Ok(())
    }
}

struct MemStore {
    runs: Mutex<HashMap<String, (String, i64)>>,
    assistant_body: Mutex<String>,
}

impl MemStore {
    fn new() -> Self {
        Self {
            runs: Mutex::new(HashMap::new()),
            assistant_body: Mutex::new(String::new()),
        }
    }
}

#[async_trait]
impl RunStore for MemStore {
    async fn create_run(
        &self,
        params: agent_core::CreateRunParams,
    ) -> Result<String, RuntimeError> {
        self.runs
            .lock()
            .unwrap()
            .insert(params.request_id.clone(), ("running".into(), 0));
        Ok("run-1".into())
    }

    async fn append_run_event(
        &self,
        _request_id: &str,
        _event_type: &str,
        _payload: &serde_json::Value,
    ) -> Result<agent_core::RunEventReceipt, RuntimeError> {
        Ok(agent_core::RunEventReceipt { id: 1 })
    }

    async fn persist_structured_message(
        &self,
        input: StructuredMessageInput,
    ) -> Result<agent_core::PersistedMessage, RuntimeError> {
        Ok(agent_core::PersistedMessage {
            id: "msg-1".into(),
            conversation_id: input.conversation_id,
            kind: input.kind,
            body: input.body,
        })
    }

    async fn update_assistant_message(
        &self,
        _message_id: &str,
        body: &str,
        _status: agent_core::MessageStatus,
        _error_message: Option<&str>,
    ) -> Result<(), RuntimeError> {
        *self.assistant_body.lock().unwrap() = body.to_string();
        Ok(())
    }

    async fn update_run(
        &self,
        request_id: &str,
        status: &str,
        _error_code: Option<&str>,
        step_count: i64,
    ) -> Result<(), RuntimeError> {
        if let Some(entry) = self.runs.lock().unwrap().get_mut(request_id) {
            *entry = (status.to_string(), step_count);
        }
        Ok(())
    }

    async fn touch_conversation_and_bot(
        &self,
        _conversation_id: &str,
        _bot_id: &str,
    ) -> Result<(), RuntimeError> {
        Ok(())
    }

    async fn get_assistant_message_body(&self, _message_id: &str) -> Result<String, RuntimeError> {
        Ok(self.assistant_body.lock().unwrap().clone())
    }
}

async fn run_with_fake(
    mode: FakeServerMode,
    cancel: Arc<AtomicBool>,
) -> (Vec<String>, String, String) {
    let process = spawn_fake_app_server_with_mode(mode)
        .await
        .expect("fake server");
    let engine = CodexRunEngine::new(CodexRunEngineConfig {
        startup_timeout: std::time::Duration::from_secs(5),
        turn_timeout: std::time::Duration::from_secs(5),
        ..CodexRunEngineConfig::default()
    });

    let events = Arc::new(RecordingEvents {
        events: Mutex::new(vec![]),
    });
    let store = Arc::new(MemStore::new());
    store
        .runs
        .lock()
        .unwrap()
        .insert("req-1".into(), ("pending".into(), 0));
    let computer = Arc::new(FakeAgentComputer::new());

    let ctx = AgentLoopContext {
        request_id: "req-1".into(),
        conversation_id: "conv-1".into(),
        assistant_message_id: "asst-1".into(),
        bot_id: "bot-1".into(),
        model: "gpt-5.6-luna".into(),
        instructions: "test".into(),
    };

    let deps = agent_core::SharedRunDeps::allow_all_approval(
        computer,
        store.clone(),
        events.clone(),
        cancel,
        "run-1".into(),
        "owner".into(),
        "comp-1".into(),
    );

    let _ = engine
        .run_with_managed_process(
            ctx,
            deps,
            vec![json!({"role":"user","content":"do work"})],
            process,
        )
        .await;

    let status = store
        .runs
        .lock()
        .unwrap()
        .get("req-1")
        .map(|(s, _)| s.clone())
        .unwrap_or_default();
    let assistant = store.assistant_body.lock().unwrap().clone();
    (events.labels(), status, assistant)
}

#[tokio::test]
async fn maps_tool_events_and_completes() {
    let (labels, status, assistant) =
        run_with_fake(FakeServerMode::HappyPath, Arc::new(AtomicBool::new(false))).await;
    assert!(labels.iter().any(|l| l == "tool_call"));
    assert!(labels.iter().any(|l| l == "tool_result"));
    assert!(labels.iter().any(|l| l == "completed"));
    assert_eq!(status, "completed");
    assert_eq!(assistant, "hello world");
}

#[tokio::test]
async fn image_bearing_turn_uses_local_image_input() {
    let process = spawn_fake_app_server_with_mode(FakeServerMode::EchoUserInput)
        .await
        .expect("fake server");
    let engine = CodexRunEngine::new(CodexRunEngineConfig {
        startup_timeout: std::time::Duration::from_secs(5),
        turn_timeout: std::time::Duration::from_secs(5),
        ..CodexRunEngineConfig::default()
    });
    let events = Arc::new(RecordingEvents {
        events: Mutex::new(vec![]),
    });
    let store = Arc::new(MemStore::new());
    store
        .runs
        .lock()
        .unwrap()
        .insert("req-1".into(), ("pending".into(), 0));
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    png.extend_from_slice(&[0; 16]);
    let descriptor = agent_core::AttachmentDescriptor {
        id: "aaaaaaaa".into(),
        original_name: "shot.png".into(),
        safe_name: "shot.png".into(),
        mime_type: "image/png".into(),
        size_bytes: png.len() as u64,
        sha256: "abc".into(),
        workspace_path: "/workspace/inputs/run-1/00-aaaaaaaa.png".into(),
        kind: agent_core::AttachmentKind::Image,
    };
    let mut deps = agent_core::SharedRunDeps::allow_all_approval(
        Arc::new(FakeAgentComputer::new()),
        store.clone(),
        events.clone(),
        Arc::new(AtomicBool::new(false)),
        "run-1".into(),
        "owner".into(),
        "comp-1".into(),
    );
    deps.attachments = Some(Arc::new(agent_core::InMemoryAttachments::new(vec![(
        descriptor.clone(),
        png,
    )])));
    deps.user_input = agent_core::RunUserInput {
        text: "describe this".into(),
        attachments: vec![descriptor],
    };
    let ctx = AgentLoopContext {
        request_id: "req-1".into(),
        conversation_id: "conv-1".into(),
        assistant_message_id: "asst-1".into(),
        bot_id: "bot-1".into(),
        model: "gpt-5.6-luna".into(),
        instructions: "test".into(),
    };
    let _ = engine
        .run_with_managed_process(
            ctx,
            deps,
            vec![json!({"role":"user","content":"describe this"})],
            process,
        )
        .await;
    let assistant = store.assistant_body.lock().unwrap().clone();
    assert!(
        assistant.contains("localImage"),
        "expected fake server to echo native image input, got {assistant}"
    );
}

#[tokio::test]
async fn ask_user_fake_turn_continues_with_selected_option() {
    let (labels, status, assistant) = run_with_fake(
        FakeServerMode::AskUserThenContinue,
        Arc::new(AtomicBool::new(false)),
    )
    .await;
    assert!(labels.iter().any(|l| l == "tool_call"));
    assert!(labels.iter().any(|l| l == "tool_result"));
    assert_eq!(status, "completed");
    assert!(assistant.contains("Staging"));
}

#[tokio::test]
async fn ignores_wrong_thread_notifications() {
    let (labels, status, _) = run_with_fake(
        FakeServerMode::WrongThreadNotifications,
        Arc::new(AtomicBool::new(false)),
    )
    .await;
    assert!(!labels.iter().any(|l| l == "tool_call"));
    assert_eq!(status, "completed");
}

#[tokio::test]
async fn failed_turn_marks_run_failed() {
    let (labels, status, _) =
        run_with_fake(FakeServerMode::TurnFailed, Arc::new(AtomicBool::new(false))).await;
    assert!(labels.iter().any(|l| l == "failed"));
    assert_eq!(status, "failed");
}
