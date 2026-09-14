use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use agent_core::{
    run_agent_loop, AgentComputer, AgentLoopContext, AgentLoopDeps, ResponsesModel, RunStore,
};
use openai_responses::OpenAiResponsesModel;
use serde_json::json;
use sprite_computer::{default_deny_network_policy, SpriteComputer, SpriteComputerConfig};
use tokio::time::timeout;

use crate::app_state::AppState;
use crate::db::postgres_run_store::PostgresRunStore;
use crate::db::queries::BootstrapRunRecords;
use crate::events::cloud_event_sink::CloudEventSink;
use crate::events::registry::ActiveRun;

#[derive(Clone)]
pub struct RunExecutionInput {
    pub records: BootstrapRunRecords,
    pub bot_id: String,
    pub user_message: String,
}

#[cfg(any(test, feature = "test-utils"))]
#[derive(Clone)]
pub struct TestRunOverrides {
    pub computer: Arc<dyn AgentComputer>,
    pub model: Arc<dyn ResponsesModel>,
}

pub fn spawn_agent_run(state: AppState, input: RunExecutionInput) {
    tokio::spawn(async move {
        let permit = match state.run_semaphore.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => return,
        };

        let cancel = Arc::new(AtomicBool::new(false));
        let (events, _rx) = CloudEventSink::new();
        let events = Arc::new(events);

        state.registry.insert(
            input.records.run_id.clone(),
            ActiveRun {
                request_id: input.records.request_id.clone(),
                cancel: cancel.clone(),
                events: events.clone(),
                started_at: std::time::Instant::now(),
            },
        );

        let pool = state.pool.clone();
        let config = state.config.clone();
        let registry = state.registry.clone();
        let run_id = input.records.run_id.clone();
        let request_id = input.records.request_id.clone();
        let timeout_secs = config.run_timeout_secs;

        let result = timeout(
            Duration::from_secs(timeout_secs),
            execute_run(config, pool.clone(), input, cancel.clone(), events),
        )
        .await;

        match result {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                tracing::error!(run_id = %run_id, error = %err, "agent run failed");
            }
            Err(_) => {
                cancel.store(true, Ordering::Relaxed);
                let store = PostgresRunStore::new(pool);
                let _ = store
                    .update_run(&request_id, "interrupted", Some("run_timeout"), 0)
                    .await;
            }
        }

        drop(permit);
        registry.remove(&run_id);
    });
}

async fn execute_run(
    config: Arc<crate::config::Config>,
    pool: sqlx::PgPool,
    input: RunExecutionInput,
    cancel: Arc<AtomicBool>,
    events: Arc<CloudEventSink>,
) -> Result<(), String> {
    let store: Arc<dyn RunStore> = Arc::new(PostgresRunStore::new(pool));

    let (computer, model) = build_deps(&config, &input, cancel.clone())?;

    computer
        .ensure_ready()
        .await
        .map_err(|e| e.to_string())?;

    let ctx = AgentLoopContext {
        request_id: input.records.request_id.clone(),
        conversation_id: input.records.conversation_id.clone(),
        assistant_message_id: input.records.assistant_message_id.clone(),
        bot_id: input.bot_id.clone(),
        model: input.records.model.clone(),
        instructions: input.records.instructions.clone(),
    };

    let deps = AgentLoopDeps {
        computer,
        store,
        events,
        model,
        cancel,
    };

    run_agent_loop(
        ctx,
        deps,
        vec![json!({"role":"user","content": input.user_message})],
    )
    .await
    .map_err(|e| e.to_string())
}

fn build_deps(
    config: &crate::config::Config,
    input: &RunExecutionInput,
    cancel: Arc<AtomicBool>,
) -> Result<(Arc<dyn AgentComputer>, Arc<dyn ResponsesModel>), String> {
    #[cfg(any(test, feature = "test-utils"))]
    if let Some(o) = test_overrides() {
        return Ok((o.computer, o.model));
    }

    let sprite_name = sprite_computer::sprite_name_for_sandbox(&input.records.computer_id);
    let computer = SpriteComputer::new(SpriteComputerConfig {
        base_url: config.sprites_api_base.clone(),
        token: config.sprite_token.clone(),
        sprite_name,
        workspace_root: "/workspace".into(),
        request_timeout: Duration::from_secs(120),
        auto_create: true,
        network_policy: default_deny_network_policy(),
        exec_timeout: Duration::from_secs(60),
    })
    .map_err(|e| e.to_string())?;

    let model = OpenAiResponsesModel::new(config.openai_api_key.clone(), cancel);
    Ok((Arc::new(computer), Arc::new(model)))
}

#[cfg(any(test, feature = "test-utils"))]
fn test_overrides() -> Option<TestRunOverrides> {
    TEST_OVERRIDES.with(|cell| cell.borrow().clone())
}

#[cfg(any(test, feature = "test-utils"))]
std::thread_local! {
    static TEST_OVERRIDES: std::cell::RefCell<Option<TestRunOverrides>> = const { std::cell::RefCell::new(None) };
}

#[cfg(any(test, feature = "test-utils"))]
pub fn set_test_run_overrides(overrides: Option<TestRunOverrides>) {
    TEST_OVERRIDES.with(|cell| *cell.borrow_mut() = overrides);
}
