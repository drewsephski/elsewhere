use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use agent_core::{
    AgentComputer, AgentLoopContext, AllowAllApprovalGate, ReadinessCachedComputer, ResponsesModel,
    ResponsesRunEngine, RunEngine, RunStore, SharedRunDeps, ToolApprovalGate,
};

use crate::approval::RunScopedApprovalGate;
use crate::auth::LEGACY_LOCAL_OWNER;
use codex_provider::{CodexRunEngine, CodexRunEngineConfig};
use futures_util::FutureExt;
use openai_responses::OpenAiResponsesModel;
use serde_json::json;
use sprite_computer::SpriteComputer;
use tokio::sync::OwnedSemaphorePermit;
use tokio::time::timeout;

use crate::app_state::AppState;
use crate::codex_ops::{CodexOperationKind, CodexOpsPermit};
use crate::config::Config;
use crate::db::postgres_run_store::PostgresRunStore;
use crate::db::queries::BootstrapRunRecords;
use crate::events::cloud_event_sink::CloudEventSink;
use crate::events::registry::ActiveRun;
use crate::finalizer::{sanitize_host_error, HostFinalizer};
use crate::run_engine_select::{
    resolve_run_engine, ResolveRunEngineError, RunEngineMode, SelectedRunEngine,
};

#[derive(Clone)]
pub struct RunExecutionInput {
    pub records: BootstrapRunRecords,
    pub bot_id: String,
    pub user_message: String,
    pub engine_mode: Option<RunEngineMode>,
}

#[cfg(any(test, feature = "test-utils"))]
#[derive(Clone)]
pub struct TestRunOverrides {
    pub computer: Arc<dyn AgentComputer>,
    pub model: Arc<dyn ResponsesModel>,
}

pub fn spawn_agent_run(state: AppState, input: RunExecutionInput, permit: OwnedSemaphorePermit) {
    let mut tasks = state.run_tasks.lock().expect("run task registry poisoned");
    while tasks.try_join_next().is_some() {}
    let execution_state = state.clone();
    tasks.spawn(async move {
        let state = execution_state;
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
                if let Err(error) = finalizer
                    .finalize_host_failure(
                        "owner_lookup_failed",
                        "Could not verify work ownership",
                        0,
                    )
                    .await
                {
                    tracing::error!(error = %error, "could not finalize work");
                }
                return;
            }
        };

        let enforce_approvals =
            owner_id != LEGACY_LOCAL_OWNER || state.config.enforce_tool_approvals_internal;

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
            std::panic::AssertUnwindSafe(            execute_run(
                state.clone(),
                config,
                pool.clone(),
                store.clone(),
                state.approvals.clone(),
                input,
                cancel.clone(),
                events.clone(),
                owner_id,
                enforce_approvals,
            ))
            .catch_unwind(),
        )
        .await;

        let finalized = match result {
            Ok(Ok(Ok(computer))) => {
                let note = match timeout(Duration::from_secs(30), crate::results::collect(&pool, &run_id, computer.as_ref())).await {
                        Ok(Ok(())) => None,
                        Ok(Err(error)) => {
                            tracing::warn!(run_id = %run_id, error = %error, "result collection incomplete");
                            Some("Some results could not be saved. Check the summary and files on the computer.")
                        }
                        Err(_) => Some("Saving file results timed out. Check the summary and files on the computer."),
                    };
                if let Err(error) =
                    sqlx::query("UPDATE agent_runs SET results_note = $2 WHERE id = $1")
                        .bind(&run_id)
                        .bind(note)
                        .execute(&pool)
                        .await
                {
                    tracing::error!(run_id = %run_id, error = %error, "could not record result collection status");
                }
                Ok(())
            }
            Ok(Ok(Err(err))) => {
                tracing::error!(run_id = %run_id, error = %err, "agent run failed");
                let message = sanitize_host_error(&err);
                let code = if err.contains("ensure_ready") || err.contains("Sprite") {
                    "sprite_unavailable"
                } else if err.contains("SpriteComputer") {
                    "sprite_config_error"
                } else {
                    "host_execution_failed"
                };
                if cancel.load(Ordering::Relaxed) {
                    finalizer.finalize_host_cancelled().await
                } else {
                    finalizer.finalize_host_failure(code, &message, 0).await
                }
            }
            Ok(Err(_)) => {
                finalizer
                    .finalize_host_interrupted(
                        "worker_panicked",
                        "Work stopped unexpectedly. Review completed actions before continuing.",
                        0,
                    )
                    .await
            }
            Err(_) => {
                cancel.store(true, Ordering::Relaxed);
                finalizer.finalize_run_timeout(0).await
            }
        };
        if let Err(error) = finalized {
            tracing::error!(run_id = %run_id, error = %error, "could not finalize work");
        }
        if let Err(error) = state
            .approvals
            .cancel_pending_for_run(&run_id, "work_finished")
            .await
        {
            tracing::error!(run_id = %run_id, error = %error, "could not close remaining approvals");
        }

        if let Err(error) =
            sqlx::query("UPDATE agent_runs SET execution_released_at = NOW() WHERE id = $1")
                .bind(&run_id)
                .execute(&pool)
                .await
        {
            tracing::error!(run_id = %run_id, error = %error, "could not release computer after execution");
        }
        drop(permit);
        registry.remove(&run_id);
    });
}

async fn execute_run(
    host_state: AppState,
    config: Arc<Config>,
    pool: sqlx::PgPool,
    store: Arc<dyn RunStore>,
    approvals: crate::approval::ApprovalService,
    input: RunExecutionInput,
    cancel: Arc<AtomicBool>,
    events: Arc<CloudEventSink>,
    owner_id: String,
    enforce_approvals: bool,
) -> Result<Arc<dyn AgentComputer>, String> {
    if let Ok(created_at) = sqlx::query_scalar::<_, chrono::DateTime<chrono::Utc>>(
        "SELECT created_at FROM agent_runs WHERE id = $1",
    )
    .bind(&input.records.run_id)
    .fetch_one(&pool)
    .await
    {
        let admission_to_execution_ms =
            (chrono::Utc::now() - created_at).num_milliseconds().max(0);
        tracing::info!(
            target: "elsewhere_run_phases",
            run_id = %input.records.run_id,
            request_id = %input.records.request_id,
            admission_to_execution_ms,
            "run execution started after durable admission"
        );
    }

    let cancelled: bool =
        sqlx::query_scalar("SELECT cancel_requested FROM agent_runs WHERE id = $1")
            .bind(&input.records.run_id)
            .fetch_one(&pool)
            .await
            .map_err(|e| e.to_string())?;
    if cancelled {
        cancel.store(true, Ordering::Relaxed);
        return Err("Work cancelled before execution".into());
    }
    let mut ctx = AgentLoopContext {
        request_id: input.records.request_id.clone(),
        conversation_id: input.records.conversation_id.clone(),
        assistant_message_id: input.records.assistant_message_id.clone(),
        bot_id: input.bot_id.clone(),
        model: input.records.model.clone(),
        instructions: input.records.instructions.clone(),
    };

    let engine_mode = match input.engine_mode {
        Some(mode) => mode,
        None => effective_engine_mode(&pool, &input.bot_id, config.run_engine).await,
    };
    let profile_home = if engine_mode == RunEngineMode::Responses {
        None
    } else {
        crate::provider_profile::profile_for_owner(&pool, &config, &owner_id)
            .await
            .map_err(|e| e.to_string())?
    };
    let mut codex_run_permit: Option<CodexOpsPermit> = None;
    let codex_availability = match engine_mode {
        RunEngineMode::Responses => codex_provider::CodexSubscriptionAvailability::NotInstalled,
        RunEngineMode::Codex => codex_provider::CodexSubscriptionAvailability::Available {
            plan_type: None,
        },
        RunEngineMode::Auto => {
            let permit = host_state
                .codex_ops
                .acquire(CodexOperationKind::Probe)
                .await
                .map_err(|_| "Codex is busy on this host".to_string())?;
            let availability = crate::codex_ops::probe_subscription_with_profile(
                config.codex_executable.clone(),
                profile_home.clone(),
                &permit,
            )
            .await;
            codex_run_permit = Some(permit);
            availability
        }
    };
    let selected = match resolve_run_engine(
        engine_mode,
        config.openai_api_key.as_deref(),
        &codex_availability,
    ) {
        Ok(engine) => engine,
        Err(err) => return Err(resolve_error_to_host(err)),
    };

    let input_messages = crate::conversation::build_run_input_messages(
        &pool,
        &input.records.conversation_id,
        &input.bot_id,
        &input.records.assistant_message_id,
        &input.user_message,
        matches!(selected, SelectedRunEngine::ResponsesApi),
    )
    .await?;
    if let Err(err) = crate::conversation::advance_group_context_cursor_for_run(
        &pool,
        &input.records.conversation_id,
        &input.bot_id,
        &input.records.assistant_message_id,
    )
    .await
    {
        tracing::warn!(
            run_id = %input.records.run_id,
            error = %err,
            "could not advance group context cursor"
        );
    }

    let permitted: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM agent_runs r JOIN sandboxes s ON s.id = r.computer_id AND s.owner_id = r.owner_id WHERE r.id = $1 AND r.owner_id = $2 AND NOT r.cancel_requested AND s.state <> 'archived')")
        .bind(&input.records.run_id).bind(&owner_id).fetch_one(&pool).await.map_err(|e| e.to_string())?;
    if !permitted {
        return Err("Work was cancelled or its computer is no longer available".into());
    }
    let computer =
        build_computer(&host_state.computer_registry, &config, &pool, &input, &owner_id).await?;

    sqlx::query("UPDATE sandboxes SET state = 'active', last_used_at = NOW(), updated_at = NOW() WHERE id = $1 AND owner_id = $2 AND state <> 'archived'")
        .bind(&input.records.computer_id).bind(&owner_id).execute(&pool).await.map_err(|e| e.to_string())?;

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

    ctx.instructions.push_str(&format!(
        "\n\nComputer workspace contract:\n\
Use /workspace for working files, notes, and intermediate artifacts.\n\
When the user asks for an output they expect to retrieve — screenshot, report, generated document, downloaded asset, code artifact, etc. — \
save the final deliverable under {} using your computer tools.\n\
For screenshots: navigate if needed, call browser_screenshot, and save the PNG in that results directory, then report the saved filename.\n\
Do not claim a file exists until its tool result reports success; if a tool fails, say so with the reason from the tool result.\n\
This results directory belongs to this assignment. Downloads support up to 20 top-level files, 1 MB each, 5 MB total. \
Include a clear final summary. File creation, shell commands, and browser mutations still require approval.\n\n\
Bot collaboration:\n\
- Use bot_list to discover other Bots owned by the same user.\n\
- Use bot_delegate to hand durable work to a specialist asynchronously; it only queues work and returns immediately.\n\
- Do not delegate trivial work or repeat the same handoff unnecessarily.\n\
- Do not claim another Bot finished work just because delegation was accepted.\n\
- Cross-computer file paths are not shared; pass bounded text context only unless both Bots share a computer.",
        crate::results::output_directory(&input.records.run_id)
    ));
    let shared = SharedRunDeps {
        computer: computer.clone(),
        store,
        events: events as Arc<dyn agent_core::EventSink>,
        cancel,
        approval_gate,
        run_id: input.records.run_id.clone(),
        owner_id,
        computer_id: input.records.computer_id.clone(),
        collaboration: Some(crate::collaboration::PostgresAgentCollaboration::new(pool.clone())),
    };

    let result = match selected {
        SelectedRunEngine::CodexSubscription => {
            tracing::info!(
                engine = "codex_subscription",
                "cloud-host selected Codex engine"
            );
            let permit = match codex_run_permit {
                Some(permit) => permit,
                None => host_state
                    .codex_ops
                    .acquire(CodexOperationKind::Run)
                    .await
                    .map_err(|_| "Codex is busy on this host".to_string())?,
            };
            run_codex_engine(&config, profile_home, ctx, shared, input_messages, permit).await
        }
        SelectedRunEngine::ResponsesApi => {
            drop(codex_run_permit);
            run_responses_engine(&config, ctx, shared, input_messages).await
        }
    };
    result.map(|()| computer)
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
    permit: CodexOpsPermit,
) -> Result<(), String> {
    permit.log_child_started();
    let engine = CodexRunEngine::new(CodexRunEngineConfig {
        executable: config.codex_executable.clone(),
        profile_home,
        compact_after_completed_turns: crate::conversation::CODEX_COMPACT_COMPLETED_TURN_INTERVAL,
        ..CodexRunEngineConfig::default()
    });
    let result = engine
        .run(ctx, shared, input)
        .await
        .map_err(|e| e.to_string());
    drop(permit);
    result
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
    registry: &crate::computer_registry::ComputerRegistry,
    config: &Config,
    pool: &sqlx::PgPool,
    input: &RunExecutionInput,
    owner_id: &str,
) -> Result<Arc<dyn AgentComputer>, String> {
    #[cfg(any(test, feature = "test-utils"))]
    if let Some(o) = test_overrides() {
        return Ok(Arc::new(ReadinessCachedComputer::new(o.computer)));
    }

    let sprite = registry
        .connect_sprite(
            config,
            pool,
            owner_id,
            &input.records.computer_id,
            config.browser_enabled,
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(Arc::new(ReadinessCachedComputer::new(sprite)))
}

pub(crate) async fn sprite_resource_for_computer(
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
