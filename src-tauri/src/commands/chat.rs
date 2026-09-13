use crate::error::AppError;
use crate::models::{
    MessageRole, MessageStatus, StartChatInput, StartChatResult, StreamEventPayload,
};
use crate::openai::{stream_chat_completion, ChatMessageInput};
use crate::state::AppState;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};
use tracing::{error, info};
use uuid::Uuid;

pub const STREAM_EVENT: &str = "gptbot://chat-stream";

const CHECKPOINT_INTERVAL: Duration = Duration::from_millis(200);

struct ChatStreamPrep {
    bot_id: String,
    conversation_id: String,
    user_message_id: String,
    assistant_message_id: String,
    model: String,
    messages_for_api: Vec<ChatMessageInput>,
}

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
        let conversation = match &input.conversation_id {
            Some(id) => db.get_conversation(id)?,
            None => db.get_or_create_primary_conversation(&input.bot_id)?,
        };
        if conversation.bot_id != input.bot_id {
            return Err(AppError::Validation(
                "conversation does not belong to this bot".into(),
            ));
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
            if matches!(
                msg.status,
                MessageStatus::Error | MessageStatus::Cancelled | MessageStatus::Interrupted
            ) {
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

        ChatStreamPrep {
            bot_id: bot.id,
            conversation_id: conversation.id,
            user_message_id: user_message.id,
            assistant_message_id: assistant_message.id,
            model: bot.model,
            messages_for_api,
        }
    };

    let cancel_token = state.register_cancel_token(&request_id);

    info!(
        request_id = %request_id,
        bot_id = %input.bot_id,
        conversation_id = %prep.conversation_id,
        user_message_id = %prep.user_message_id,
        assistant_message_id = %prep.assistant_message_id,
        "chat stream started"
    );

    let result = StartChatResult {
        request_id: request_id.clone(),
        conversation_id: prep.conversation_id.clone(),
        user_message_id: prep.user_message_id.clone(),
        assistant_message_id: prep.assistant_message_id.clone(),
    };

    let app_handle = app.clone();
    let bot_id_touch = prep.bot_id.clone();
    let conversation_id = prep.conversation_id.clone();
    let assistant_message_id = prep.assistant_message_id.clone();
    let model = prep.model.clone();
    let messages_for_api = prep.messages_for_api;

    tauri::async_runtime::spawn(async move {
        let emit = |payload: StreamEventPayload| {
            let _ = app_handle.emit(STREAM_EVENT, payload);
        };

        let assistant_id = assistant_message_id.clone();
        let conv_id = conversation_id.clone();
        let req_id = request_id.clone();

        let mut live_body = String::new();
        let mut last_checkpoint = Instant::now();

        let stream_result = stream_chat_completion(
            &api_key,
            &model,
            messages_for_api,
            cancel_token,
            |delta| {
                live_body.push_str(delta);
                let should_checkpoint = last_checkpoint.elapsed() >= CHECKPOINT_INTERVAL;
                if should_checkpoint {
                    let state = app_handle.state::<AppState>();
                    let db = state.db.lock();
                    if let Err(err) = db.update_message_body_and_status(
                        &assistant_id,
                        &live_body,
                        MessageStatus::Streaming,
                        None,
                    ) {
                        error!(
                            request_id = %req_id,
                            error = %err,
                            "stream checkpoint failed"
                        );
                    }
                    last_checkpoint = Instant::now();
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
                let finalize_result = {
                    let db = state.db.lock();
                    db.update_message_body_and_status(
                        &assistant_id,
                        &full,
                        MessageStatus::Complete,
                        None,
                    )
                    .and_then(|_| db.touch_conversation(&conv_id))
                    .and_then(|_| db.touch_bot(&bot_id_touch))
                };
                match finalize_result {
                    Ok(_) => {
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
                    Err(err) => {
                        error!(
                            request_id = %req_id,
                            error = %err,
                            "chat stream finalization failed"
                        );
                        emit(StreamEventPayload {
                            request_id: req_id.clone(),
                            event_type: "error".to_string(),
                            conversation_id: conv_id.clone(),
                            assistant_message_id: assistant_id.clone(),
                            delta: None,
                            error: Some(err.to_string()),
                            full_content: Some(full),
                        });
                    }
                }
            }
            Err(AppError::Cancelled) => {
                let partial = live_body;
                if let Err(err) = {
                    let db = state.db.lock();
                    db.update_message_body_and_status(
                        &assistant_id,
                        &partial,
                        MessageStatus::Cancelled,
                        None,
                    )
                } {
                    error!(request_id = %req_id, error = %err, "cancel persist failed");
                }
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
                let partial = live_body;
                if let Err(persist_err) = {
                    let db = state.db.lock();
                    db.update_message_body_and_status(
                        &assistant_id,
                        &partial,
                        MessageStatus::Error,
                        Some(&message),
                    )
                } {
                    error!(request_id = %req_id, error = %persist_err, "error persist failed");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::models::CreateBotInput;

    #[test]
    fn rejects_foreign_conversation_for_bot() {
        let db = Database::open_in_memory().expect("db");
        let bot_a = db
            .create_bot(CreateBotInput {
                name: "A".into(),
                description: None,
                system_prompt: None,
                provider: None,
                model: "gpt-4o-mini".into(),
            })
            .expect("bot a");
        let bot_b = db
            .create_bot(CreateBotInput {
                name: "B".into(),
                description: None,
                system_prompt: None,
                provider: None,
                model: "gpt-4o-mini".into(),
            })
            .expect("bot b");
        let conv_a = db.create_conversation(&bot_a.id, None).expect("conv");
        let conversation = db.get_conversation(&conv_a.id).expect("get");
        assert_ne!(conversation.bot_id, bot_b.id);
    }
}
