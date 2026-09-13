use crate::error::AppError;
use crate::models::Message;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn list_messages(
    state: State<AppState>,
    conversation_id: String,
) -> Result<Vec<Message>, AppError> {
    let db = state.db.lock();
    db.list_messages(&conversation_id)
}
