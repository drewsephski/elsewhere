use crate::error::AppError;
use crate::models::Conversation;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn list_conversations(
    state: State<AppState>,
    bot_id: String,
) -> Result<Vec<Conversation>, AppError> {
    let db = state.db.lock();
    db.list_conversations(&bot_id)
}

#[tauri::command]
pub fn get_or_create_conversation(
    state: State<AppState>,
    bot_id: String,
) -> Result<Conversation, AppError> {
    let db = state.db.lock();
    db.get_or_create_primary_conversation(&bot_id)
}
