use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use agent_core::{
    AgentComputer, AgentLoopContext, AllowAllApprovalGate, ResponsesModel, ResponsesRunEngine,
    RunEngine, RunStore, SharedRunDeps, ToolApprovalGate,
};

use crate::approval::RunScopedApprovalGate;
use crate::auth::LEGACY_LOCAL_OWNER;
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
use codex_provider::probe_codex_subscription_availability_with_profile;

use crate::run_engine_select::{
    resolve_run_engine, ResolveRunEngineError, RunEngineMode, SelectedRunEngine,
};

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

pub fn spawn_agent_run(state: AppState, input: RunExecutionInput, permit: OwnedSemaphorePermit) {
    tokio::spawn(async move {
        let cancel = Arc::new(AtomicBool::new(false));
        let (events, _rx) = CloudEventSink::new();
        let events = Arc::new(events);

        let pool = state.pool.clone();
        let store: Arc<dyn RunStore> = Arc::new(PostgresRunStore::new(pool.clone()));

        let owner_id: String = match sqlx::query_scalar(
            "SELECT owner_id FROM agent_runs WHERE id = $1",
        )
        .bind(&input.records.run_id)
        .fetch_one(&pool)
        .await
        {
            Ok(owner_id) => owner_id,
            Err(err) => {
                tracing::error!(run_id = %input.records.run_id, error = %err, "cannot resolve run owner; refusing execution");
                return;
            }
        };

        let enforce_approvals =
            owner_id != LEGACY_LOCAL_OWNER || state.config.enforce_tool_approvals_internal;

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
            execute_run(
                config,
                pool.clone(),
                store.clone(),
                state.approvals.clone(),
                input,
                cancel.clone(),
                events.clone(),
                owner_id,
                enforce_approvals,
            ),
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
    approvals: crate::approval::ApprovalService,
    input: RunExecutionInput,
    cancel: Arc<AtomicBool>,
    events: Arc<CloudEventSink>,
    owner_id: String,
    enforce_approvals: bool,
) -> Result<(), String> {
    let ctx = AgentLoopContext {
        request_id: input.records.request_id.clone(),
        conversation_id: input.records.conversation_id.clone(),
        assistant_message_id: input.records.assistant_message_id.clone(),
        bot_id: input.bot_id.clone(),
        model: input.records.model.clone(),
        instructions: input.records.instructions.clone(),
    };

    let input_messages = vec![json!({"role":"user","content": input.user_message})];

    let engine_mode = effective_engine_mode(&pool, &input.bot_id, config.run_engine).await;
    let profile_home = if engine_mode == RunEngineMode::Responses {
        None
    } else {
        crate::provider_profile::profile_for_owner(&pool, &config, &owner_id)
            .await
            .map_err(|e| e.to_string())?
    };
    let codex_availability = if engine_mode == RunEngineMode::Responses {
        codex_provider::CodexSubscriptionAvailability::NotInstalled
    } else {
        probe_codex_subscription_availability_with_profile(
            config.codex_executable.clone(),
            profile_home.clone(),
        )
        .await
    };
    let selected = match resolve_run_engine(
        engine_mode,
        config.openai_api_key.as_deref(),
        &codex_availability,
    ) {
        Ok(engine) => engine,
        Err(err) => return Err(resolve_error_to_host(err)),
    };

    let computer = build_computer(&config, &pool, &input).await?;

    computer
        .ensure_ready()
        .await
        .map_err(|e| format!("ensure_ready: {e}"))?;

    let approval_gate: Arc<dyn ToolApprovalGate> = if enforce_approvals {
        Arc::new(RunScopedApprovalGate::new(
            approvals,
            events.clone(),
            store.clone(),
            cancel.clone(),
        ))
    } else {
        Arc::new(AllowAllApprovalGate)
    };

    let shared = SharedRunDeps {
        computer,
        store,
        events: events as Arc<dyn agent_core::EventSink>,
        cancel,
        approval_gate,
        run_id: input.records.run_id.clone(),
        owner_id,
        computer_id: input.records.computer_id.clone(),
    };

    match selected {
        SelectedRunEngine::CodexSubscription => {
            tracing::info!(
                engine = "codex_subscription",
                "cloud-host selected Codex engine"
            );
            run_codex_engine(&config, profile_home, ctx, shared, input_messages).await
        }
        SelectedRunEngine::ResponsesApi => {
            run_responses_engine(&config, ctx, shared, input_messages).await
        }
    }
}

fn resolve_error_to_host(err: ResolveRunEngineError) -> String {
    let code = match &err {
        ResolveRunEngineError::NoModelProviderAvailable => "no_model_provider_available",
        ResolveRunEngineError::ResponsesApiKeyRequired => "responses_api_key_required",
        ResolveRunEngineError::CodexUnavailable(_) => "codex_unavailable",
    };
    format!("{code}: {}", err.as_run_message())
}

async fn run_codex_engine(
    config: &Config,
    profile_home: Option<std::path::PathBuf>,
    ctx: AgentLoopContext,
    shared: SharedRunDeps,
    input: Vec<serde_json::Value>,
) -> Result<(), String> {
    let engine = CodexRunEngine::new(CodexRunEngineConfig {
        executable: config.codex_executable.clone(),
        profile_home,
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
    #[cfg(any(test, feature = "test-utils"))]
    let model: Arc<dyn ResponsesModel> = if let Some(o) = test_overrides() {
        o.model
    } else {
        let api_key = config
            .openai_api_key
            .clone()
            .ok_or_else(|| "OPENAI_API_KEY is required for the Responses engine".to_string())?;
        Arc::new(OpenAiResponsesModel::new(api_key, shared.cancel.clone()))
    };

    #[cfg(not(any(test, feature = "test-utils")))]
    let model: Arc<dyn ResponsesModel> = {
        let api_key = config
            .openai_api_key
            .clone()
            .ok_or_else(|| "OPENAI_API_KEY is required for the Responses engine".to_string())?;
        Arc::new(OpenAiResponsesModel::new(api_key, shared.cancel.clone()))
    };

    tracing::info!(
        engine = "responses_api",
        "cloud-host selected Responses engine"
    );
    let engine = ResponsesRunEngine::new(model);
    engine
        .run(ctx, shared, input)
        .await
        .map_err(|e| e.to_string())
}

async fn effective_engine_mode(
    pool: &sqlx::PgPool,
    bot_id: &str,
    host_default: RunEngineMode,
) -> RunEngineMode {
    let row: Option<(String,)> = sqlx::query_as("SELECT engine_preference FROM bots WHERE id = $1")
        .bind(bot_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
    let Some((pref,)) = row else {
        return host_default;
    };
    crate::run_engine_select::parse_run_engine_mode(&pref).unwrap_or(host_default)
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
    let row: Option<(String,)> =
        sqlx::query_as("SELECT provider_resource_id FROM sandboxes WHERE id = $1")
            .bind(computer_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
    if let Some((name,)) = row {
        if !name.is_empty() {
            return Ok(name);
        }
    }
    Ok(sprite_computer::sprite_name_for_sandbox(computer_id))
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
