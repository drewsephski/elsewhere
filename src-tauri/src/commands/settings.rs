use crate::error::AppError;
use crate::models::ModelDescriptor;
use crate::openai::OpenAiClient;
use crate::state::AppState;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyStatus {
    pub configured: bool,
}

#[tauri::command]
pub fn get_api_key_status(state: State<AppState>) -> Result<ApiKeyStatus, AppError> {
    let configured = state.secrets.has_openai_api_key()?;
    Ok(ApiKeyStatus { configured })
}

#[tauri::command]
pub fn set_openai_api_key(state: State<AppState>, api_key: String) -> Result<(), AppError> {
    state.secrets.set_openai_api_key(&api_key)?;
    Ok(())
}

#[tauri::command]
pub fn clear_openai_api_key(state: State<AppState>) -> Result<(), AppError> {
    state.secrets.delete_openai_api_key()?;
    Ok(())
}

#[tauri::command]
pub async fn list_openai_models(state: State<'_, AppState>) -> Result<Vec<ModelDescriptor>, AppError> {
    let api_key = state
        .secrets
        .get_openai_api_key()?
        .ok_or(AppError::MissingApiKey)?;
    let client = OpenAiClient::new(api_key);
    client.list_models().await
}
