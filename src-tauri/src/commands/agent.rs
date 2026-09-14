use crate::agent::{
    build_responses_input_from_messages, run_agent_chat, AgentLoopContext, AgentLoopDeps,
    LocalMacComputer, OpenAiResponsesModel, SqliteRunStore, TauriEventSink,
};
use crate::error::AppError;
use crate::models::{MessageRole, MessageStatus, StartChatInput, StartChatResult};
use crate::openai::model_supports_responses_tools;
use crate::state::AppState;
use agent_core::{CreateRunParams, RunStore};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tracing::{error, info};
use uuid::Uuid;

#[tauri::command]
pub async fn start_agent_chat(
    app: AppHandle,
    state: State<'_, AppState>,
    input: StartChatInput,
) -> Result<StartChatResult, AppError> {
    let api_key = state
        .secrets
        .get_openai_api_key()?
        .ok_or(AppError::MissingApiKey)?;

    let request_id = match &input.request_id {
        Some(id) if !id.trim().is_empty() => id.trim().to_string(),
        _ => Uuid::new_v4().to_string(),
    };

    let content = input.content.trim();
    if content.is_empty() {
        return Err(AppError::Validation("message cannot be empty".into()));
    }

    let prep = {
        let db = state.db.lock();
        let bot = db.get_bot(&input.bot_id)?;
        if !bot.computer_enabled {
            return Err(AppError::Validation(
                "This bot is chat-only and cannot use the agent computer runtime.".into(),
            ));
        }
        if !model_supports_responses_tools(&bot.model) {
            return Err(AppError::ModelUnavailable(format!(
                "Model {} does not support Elsewhere local tools (OpenAI Responses function calling)",
                bot.model
            )));
        }
        let conversation = match &input.conversation_id {
            Some(id) => db.get_conversation_for_bot(id, &input.bot_id)?,
            None => db.get_or_create_primary_conversation(&input.bot_id)?,
        };

        let user_message = db.insert_message(
            &conversation.id,
            MessageRole::User,
            content,
            MessageStatus::Complete,
            None,
        )?;

        let assistant_message = db.insert_message(
            &conversation.id,
            MessageRole::Assistant,
            "",
            MessageStatus::Streaming,
            Some(&bot.model),
        )?;

        let history = db.list_messages(&conversation.id)?;
        let input_items = build_responses_input_from_messages(&history)?;

        (
            bot.id,
            bot.model,
            bot.system_prompt,
            conversation.id,
            user_message.id,
            assistant_message.id,
            input_items,
        )
    };

    let (
        bot_id,
        model,
        system_prompt,
        conversation_id,
        user_message_id,
        assistant_message_id,
        input_items,
    ) = prep;

    let cancel_token = state.register_cancel_token(&request_id);
    let store = SqliteRunStore::new(state.db.clone());
    store
        .create_run(CreateRunParams {
            conversation_id: conversation_id.clone(),
            request_id: request_id.clone(),
            bot_id: bot_id.clone(),
            model: model.clone(),
            computer_id: Some("local-mac".into()),
        })
        .map_err(|e| AppError::Other(e.to_string()))?;

    info!(
        request_id = %request_id,
        bot_id = %input.bot_id,
        conversation_id = %conversation_id,
        "agent run started"
    );

    let result = StartChatResult {
        request_id: request_id.clone(),
        conversation_id: conversation_id.clone(),
        user_message_id,
        assistant_message_id: assistant_message_id.clone(),
    };

    let app_handle = app.clone();
    let vm = state.vm.clone();
    let db = state.db.clone();

    tauri::async_runtime::spawn(async move {
        let ctx = AgentLoopContext {
            request_id: request_id.clone(),
            conversation_id,
            assistant_message_id,
            bot_id,
            model,
            instructions: system_prompt,
        };

        let deps = AgentLoopDeps {
            computer: Arc::new(LocalMacComputer::new(vm)),
            store: Arc::new(SqliteRunStore::new(db)),
            events: Arc::new(TauriEventSink::new(
                app_handle.clone(),
                request_id.clone(),
                ctx.conversation_id.clone(),
                ctx.assistant_message_id.clone(),
            )),
            model: Arc::new(OpenAiResponsesModel::new(api_key, cancel_token.clone())),
            cancel: cancel_token.clone(),
        };

        if let Err(err) = run_agent_chat(deps, ctx, input_items).await {
            error!(request_id = %request_id, error = %err, "agent run task failed");
        }

        app_handle
            .state::<AppState>()
            .remove_cancel_token(&request_id);
    });

    Ok(result)
}
