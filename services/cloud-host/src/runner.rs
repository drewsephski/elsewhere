use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use agent_core::{
    AgentComputer, AgentLoopContext, ResponsesModel, ResponsesRunEngine, RunEngine, RunStore,
    SharedRunDeps,
};
use codex_provider::{CodexRunEngine, CodexRunEngineConfig};
use openai_responses::OpenAiResponsesModel;
use serde_json::json;
use sprite_computer::{default_deny_network_policy, SpriteComputer, SpriteComputerConfig};
use tokio::sync::OwnedSemaphorePermit;
use tokio::time::timeout;

use crate::app_state::AppState;
use crate::config::Config;
use crate::db::postgres_run_store::PostgresRunStore;
use crate::db::queries::BootstrapRunRecords;
use crate::events::cloud_event_sink::CloudEventSink;
use crate::events::registry::ActiveRun;
use crate::finalizer::{sanitize_host_error, HostFinalizer};
use crate::run_engine_select::{auto_fallback_from_codex_error, RunEngineMode};

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

pub fn spawn_agent_run(
    state: AppState,
    input: RunExecutionInput,
    permit: OwnedSemaphorePermit,
) {
    tokio::spawn(async move {
        let cancel = Arc::new(AtomicBool::new(false));
        let (events, _rx) = CloudEventSink::new();
        let events = Arc::new(events);

        let pool = state.pool.clone();
        let store: Arc<dyn RunStore> = Arc::new(PostgresRunStore::new(pool.clone()));

        let finalizer = HostFinalizer::new(
            store.clone(),
            events.clone(),
            input.records.request_id.clone(),
            input.records.assistant_message_id.clone(),
        );

        if !state.registry.try_begin_run(
            &input.records.run_id,
            ActiveRun {
                request_id: input.records.request_id.clone(),
                cancel: cancel.clone(),
                events: events.clone(),
                started_at: std::time::Instant::now(),
            },
        ) {
            drop(permit);
            return;
        }

        let config = state.config.clone();
        let registry = state.registry.clone();
        let run_id = input.records.run_id.clone();
        let timeout_secs = config.run_timeout_secs;

        let result = timeout(
            Duration::from_secs(timeout_secs),
            execute_run(config, pool.clone(), store.clone(), input, cancel.clone(), events.clone()),
        )
        .await;

        match result {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                tracing::error!(run_id = %run_id, error = %err, "agent run failed");
                let message = sanitize_host_error(&err);
                let code = if err.contains("ensure_ready") || err.contains("Sprite") {
                    "sprite_unavailable"
                } else if err.contains("SpriteComputer") {
                    "sprite_config_error"
                } else {
                    "host_execution_failed"
                };
                let _ = finalizer.finalize_host_failure(code, &message, 0).await;
            }
            Err(_) => {
                cancel.store(true, Ordering::Relaxed);
                let _ = finalizer.finalize_run_timeout(0).await;
            }
        }

        drop(permit);
        registry.remove(&run_id);
    });
}

async fn execute_run(
    config: Arc<Config>,
    pool: sqlx::PgPool,
    store: Arc<dyn RunStore>,
    input: RunExecutionInput,
    cancel: Arc<AtomicBool>,
    events: Arc<CloudEventSink>,
) -> Result<(), String> {
    let computer = build_computer(&config, &pool, &input).await?;

    computer
        .ensure_ready()
        .await
        .map_err(|e| format!("ensure_ready: {e}"))?;

    let ctx = AgentLoopContext {
        request_id: input.records.request_id.clone(),
        conversation_id: input.records.conversation_id.clone(),
        assistant_message_id: input.records.assistant_message_id.clone(),
        bot_id: input.bot_id.clone(),
        model: input.records.model.clone(),
        instructions: input.records.instructions.clone(),
    };

    let input_messages = vec![json!({"role":"user","content": input.user_message})];

    match config.run_engine {
        RunEngineMode::Codex => {
            tracing::info!(engine = "codex_subscription", "cloud-host selected Codex engine");
            let shared = SharedRunDeps {
                computer,
                store,
                events: events as Arc<dyn agent_core::EventSink>,
                cancel,
            };
            run_codex_engine(&config, ctx, shared, input_messages).await
        }
        RunEngineMode::Responses => {
            let shared = SharedRunDeps {
                computer,
                store,
                events: events as Arc<dyn agent_core::EventSink>,
                cancel,
            };
            run_responses_engine(&config, ctx, shared, input_messages).await
        }
        RunEngineMode::Auto => {
            let shared_codex = SharedRunDeps {
                computer: computer.clone(),
                store: store.clone(),
                events: events.clone() as Arc<dyn agent_core::EventSink>,
                cancel: cancel.clone(),
            };
            let codex_result =
                run_codex_engine(&config, ctx, shared_codex, input_messages.clone()).await;
            if codex_result.is_ok() {
                return Ok(());
            }
            let err = codex_result.unwrap_err();
            if auto_fallback_from_codex_error(&err) && config.openai_api_key.is_some() {
                tracing::warn!(
                    engine = "auto",
                    reason = %err,
                    "falling back to Responses API engine"
                );
                let ctx_responses = AgentLoopContext {
                    request_id: input.records.request_id.clone(),
                    conversation_id: input.records.conversation_id.clone(),
                    assistant_message_id: input.records.assistant_message_id.clone(),
                    bot_id: input.bot_id.clone(),
                    model: input.records.model.clone(),
                    instructions: input.records.instructions.clone(),
                };
                let shared_responses = SharedRunDeps {
                    computer,
                    store,
                    events: events as Arc<dyn agent_core::EventSink>,
                    cancel,
                };
                return run_responses_engine(&config, ctx_responses, shared_responses, input_messages)
                    .await;
            }
            Err(err)
        }
    }
}

async fn run_codex_engine(
    config: &Config,
    ctx: AgentLoopContext,
    shared: SharedRunDeps,
    input: Vec<serde_json::Value>,
) -> Result<(), String> {
    let engine = CodexRunEngine::new(CodexRunEngineConfig {
        executable: config.codex_executable.clone(),
        ..CodexRunEngineConfig::default()
    });
    engine
        .run(ctx, shared, input)
        .await
        .map_err(|e| e.to_string())
}

async fn run_responses_engine(
    config: &Config,
    ctx: AgentLoopContext,
    shared: SharedRunDeps,
    input: Vec<serde_json::Value>,
) -> Result<(), String> {
    let api_key = config
        .openai_api_key
        .clone()
        .ok_or_else(|| "OPENAI_API_KEY is required for the Responses engine".to_string())?;
    tracing::info!(engine = "responses_api", "cloud-host selected Responses engine");
    let model = OpenAiResponsesModel::new(api_key, shared.cancel.clone());
    let engine = ResponsesRunEngine::new(Arc::new(model));
    engine
        .run(ctx, shared, input)
        .await
        .map_err(|e| e.to_string())
}

async fn build_computer(
    config: &Config,
    pool: &sqlx::PgPool,
    input: &RunExecutionInput,
) -> Result<Arc<dyn AgentComputer>, String> {
    #[cfg(any(test, feature = "test-utils"))]
    if let Some(o) = test_overrides() {
        return Ok(o.computer);
    }

    let sprite_name = sprite_resource_for_computer(pool, &input.records.computer_id).await?;
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
    .map_err(|e| format!("SpriteComputer: {e}"))?;

    Ok(Arc::new(computer))
}

async fn sprite_resource_for_computer(
    pool: &sqlx::PgPool,
    computer_id: &str,
) -> Result<String, String> {
    let sandbox_id = format!("sandbox-{computer_id}");
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT provider_resource_id FROM sandboxes WHERE id = $1",
    )
    .bind(&sandbox_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(row
        .map(|(name,)| name)
        .unwrap_or_else(|| sprite_computer::sprite_name_for_sandbox(computer_id)))
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
