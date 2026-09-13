use crate::error::AppError;
use crate::models::{
    MessageRole, MessageStatus, StartChatInput, StartChatResult, StreamEventPayload,
};
use crate::openai::{stream_chat_completion, ChatMessageInput};
use crate::state::AppState;
use tauri::{AppHandle, Emitter, Manager, State};
use tracing::{error, info};
use uuid::Uuid;

pub const STREAM_EVENT: &str = "gptbot://chat-stream";

#[tauri::command]
pub async fn start_chat(
    app: AppHandle,
    state: State<'_, AppState>,
    input: StartChatInput,
) -> Result<StartChatResult, AppError> {
    let api_key = state
        .secrets
        .get_openai_api_key()?
        .ok_or(AppError::MissingApiKey)?;

    let request_id = Uuid::new_v4().to_string();
    let cancel_token = state.register_cancel_token(&request_id);

    let (
        _bot,
        bot_id_for_touch,
        conversation_id,
        user_message_id,
        assistant_message_id,
        model,
        messages_for_api,
    ) = {
        let db = state.db.lock();
        let bot = db.get_bot(&input.bot_id)?;
        let conversation = match &input.conversation_id {
            Some(id) => db.get_conversation(id)?,
            None => db.get_or_create_primary_conversation(&input.bot_id)?,
        };
        let content = input.content.trim();
        if content.is_empty() {
            return Err(AppError::Validation("message cannot be empty".into()));
        }

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
        let mut messages_for_api: Vec<ChatMessageInput> = Vec::new();
        if !bot.system_prompt.trim().is_empty() {
            messages_for_api.push(ChatMessageInput {
                role: "system".to_string(),
                content: bot.system_prompt.clone(),
            });
        }
        for msg in history {
            if msg.id == assistant_message.id {
                continue;
            }
            if msg.status == MessageStatus::Error || msg.status == MessageStatus::Cancelled {
                continue;
            }
            if msg.body.trim().is_empty() && msg.role != MessageRole::User {
                continue;
            }
            let role = match msg.role {
                MessageRole::System => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
            };
            messages_for_api.push(ChatMessageInput {
                role: role.to_string(),
                content: msg.body.clone(),
            });
        }

        let model = bot.model.clone();
        let bot_id = bot.id.clone();
        (
            bot,
            bot_id,
            conversation.id,
            user_message.id,
            assistant_message.id,
            model,
            messages_for_api,
        )
    };

    info!(
        request_id = %request_id,
        bot_id = %input.bot_id,
        conversation_id = %conversation_id,
        user_message_id = %user_message_id,
        assistant_message_id = %assistant_message_id,
        "chat stream started"
    );

    let result = StartChatResult {
        request_id: request_id.clone(),
        conversation_id: conversation_id.clone(),
        user_message_id,
        assistant_message_id: assistant_message_id.clone(),
    };

    let app_handle = app.clone();

    let bot_id_touch = bot_id_for_touch.clone();
    tauri::async_runtime::spawn(async move {
        let emit = |payload: StreamEventPayload| {
            let _ = app_handle.emit(STREAM_EVENT, payload);
        };

        let assistant_id = assistant_message_id.clone();
        let conv_id = conversation_id.clone();
        let req_id = request_id.clone();

        let stream_result = stream_chat_completion(
            &api_key,
            &model,
            messages_for_api,
            cancel_token,
            |delta| {
                {
                    let state = app_handle.state::<AppState>();
                    let db = state.db.lock();
                    let _ = db.append_message_body(&assistant_id, delta);
                }
                emit(StreamEventPayload {
                    request_id: req_id.clone(),
                    event_type: "delta".to_string(),
                    conversation_id: conv_id.clone(),
                    assistant_message_id: assistant_id.clone(),
                    delta: Some(delta.to_string()),
                    error: None,
                    full_content: None,
                });
            },
        )
        .await;

        let state = app_handle.state::<AppState>();

        match stream_result {
            Ok(full) => {
                {
                    let db = state.db.lock();
                    let _ = db.update_message_body_and_status(
                        &assistant_id,
                        &full,
                        MessageStatus::Complete,
                        None,
                    );
                    let _ = db.touch_conversation(&conv_id);
                    let _ = db.touch_bot(&bot_id_touch);
                }
                emit(StreamEventPayload {
                    request_id: req_id.clone(),
                    event_type: "done".to_string(),
                    conversation_id: conv_id.clone(),
                    assistant_message_id: assistant_id.clone(),
                    delta: None,
                    error: None,
                    full_content: Some(full),
                });
                info!(request_id = %req_id, "chat stream completed");
            }
            Err(AppError::Cancelled) => {
                let partial = {
                    let db = state.db.lock();
                    let msg = db.get_message(&assistant_id).ok();
                    if let Some(ref m) = msg {
                        let _ = db.update_message_body_and_status(
                            &assistant_id,
                            &m.body,
                            MessageStatus::Cancelled,
                            None,
                        );
                    }
                    msg.map(|m| m.body).unwrap_or_default()
                };
                emit(StreamEventPayload {
                    request_id: req_id.clone(),
                    event_type: "cancelled".to_string(),
                    conversation_id: conv_id.clone(),
                    assistant_message_id: assistant_id.clone(),
                    delta: None,
                    error: None,
                    full_content: Some(partial),
                });
            }
            Err(err) => {
                error!(request_id = %req_id, error = %err, "chat stream failed");
                let message = err.to_string();
                {
                    let db = state.db.lock();
                    let partial = db.get_message(&assistant_id).map(|m| m.body).unwrap_or_default();
                    let _ = db.update_message_body_and_status(
                        &assistant_id,
                        &partial,
                        MessageStatus::Error,
                        Some(&message),
                    );
                }
                emit(StreamEventPayload {
                    request_id: req_id.clone(),
                    event_type: "error".to_string(),
                    conversation_id: conv_id.clone(),
                    assistant_message_id: assistant_id.clone(),
                    delta: None,
                    error: Some(message),
                    full_content: None,
                });
            }
        }

        state.remove_cancel_token(&req_id);
    });

    Ok(result)
}

#[tauri::command]
pub fn cancel_chat(state: State<AppState>, request_id: String) -> Result<(), AppError> {
    if state.cancel_request(&request_id) {
        info!(request_id = %request_id, "chat stream cancel requested");
        Ok(())
    } else {
        Err(AppError::NotFound(format!("stream {}", request_id)))
    }
}
