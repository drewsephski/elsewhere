use crate::error::AppError;
use crate::models::{Bot, CreateBotInput, UpdateBotInput};
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn list_bots(
    state: State<AppState>,
    include_archived: Option<bool>,
) -> Result<Vec<Bot>, AppError> {
    let db = state.db.lock();
    db.list_bots(include_archived.unwrap_or(false))
}

#[tauri::command]
pub fn get_bot(state: State<AppState>, id: String) -> Result<Bot, AppError> {
    let db = state.db.lock();
    db.get_bot(&id)
}

#[tauri::command]
pub fn create_bot(state: State<AppState>, input: CreateBotInput) -> Result<Bot, AppError> {
    let db = state.db.lock();
    db.create_bot(input)
}

#[tauri::command]
pub fn update_bot(state: State<AppState>, input: UpdateBotInput) -> Result<Bot, AppError> {
    let db = state.db.lock();
    db.update_bot(input)
}

#[tauri::command]
pub fn archive_bot(state: State<AppState>, id: String) -> Result<(), AppError> {
    let db = state.db.lock();
    db.archive_bot(&id)
}

#[tauri::command]
pub fn delete_bot(state: State<AppState>, id: String) -> Result<(), AppError> {
    let db = state.db.lock();
    db.delete_bot(&id)
}
