#![cfg(feature = "test-utils")]

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use agent_core::{
    AgentEvent, AgentLoopContext, EventSink, FakeAgentComputer, RuntimeError, RunStore,
    StructuredMessageInput,
};
use async_trait::async_trait;
use codex_provider::{
    spawn_fake_app_server_with_mode, CodexRunEngine, CodexRunEngineConfig, FakeServerMode,
};
use serde_json::json;

struct RecordingStore {
    events: Mutex<Vec<(String, serde_json::Value)>>,
    assistant_body: Mutex<String>,
    runs: Mutex<HashMap<String, (String, i64)>>,
}

impl RecordingStore {
    fn event_types(&self) -> Vec<String> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .map(|(t, _)| t.clone())
            .collect()
    }
}

#[async_trait]
impl RunStore for RecordingStore {
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
        event_type: &str,
        payload: &serde_json::Value,
    ) -> Result<agent_core::RunEventReceipt, RuntimeError> {
        let id = self.events.lock().unwrap().len() as i64 + 1;
        self.events
            .lock()
            .unwrap()
            .push((event_type.to_string(), payload.clone()));
        Ok(agent_core::RunEventReceipt { id })
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

struct RecordingEvents {
    durable: Mutex<Vec<String>>,
}

impl EventSink for RecordingEvents {
    fn emit(&self, event: AgentEvent) -> Result<(), RuntimeError> {
        let _ = event;
        Ok(())
    }

    fn emit_durable(
        &self,
        _event_id: i64,
        event_type: &str,
        _payload: &serde_json::Value,
    ) -> Result<(), RuntimeError> {
        self.durable.lock().unwrap().push(event_type.to_string());
        Ok(())
    }
}

#[tokio::test]
async fn assistant_deltas_persist_before_terminal() {
    let process = spawn_fake_app_server_with_mode(FakeServerMode::TextOnly)
        .await
        .expect("fake server");
    let engine = CodexRunEngine::new(CodexRunEngineConfig {
        startup_timeout: std::time::Duration::from_secs(5),
        turn_timeout: std::time::Duration::from_secs(5),
        ..CodexRunEngineConfig::default()
    });
    let store = Arc::new(RecordingStore {
        events: Mutex::new(vec![]),
        assistant_body: Mutex::new(String::new()),
        runs: Mutex::new(HashMap::new()),
    });
    store
        .runs
        .lock()
        .unwrap()
        .insert("req-1".into(), ("pending".into(), 0));
    let computer = Arc::new(FakeAgentComputer::new());
    let events = Arc::new(RecordingEvents {
        durable: Mutex::new(vec![]),
    });
    let ctx = AgentLoopContext {
        request_id: "req-1".into(),
        conversation_id: "conv-1".into(),
        assistant_message_id: "asst-1".into(),
        bot_id: "bot-1".into(),
        model: "gpt-5.6-luna".into(),
        instructions: "test".into(),
    };
    let deps = agent_core::SharedRunDeps::allow_all_approval(
        computer.clone(),
        store.clone(),
        events,
        Arc::new(AtomicBool::new(false)),
        "run-1".into(),
        "owner".into(),
        "comp-1".into(),
    );

    let _ = engine
        .run_with_managed_process(
            ctx,
            deps,
            vec![json!({"role":"user","content":"hi"})],
            process,
        )
        .await;

    let types = store.event_types();
    let delta_idx = types.iter().position(|t| t == "assistant_delta");
    let terminal_idx = types.iter().position(|t| t == "terminal");
    assert!(delta_idx.is_some());
    assert!(terminal_idx.is_some());
    assert!(delta_idx.unwrap() < terminal_idx.unwrap());
    assert_eq!(store.assistant_body.lock().unwrap().as_str(), "hello world");
    assert_eq!(computer.ensure_ready_calls(), 0);
}
