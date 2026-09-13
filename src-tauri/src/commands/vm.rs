use crate::error::AppError;
use crate::state::AppState;
use crate::vm::{GuestRequest, GuestResponse, VmInfo};
use tauri::State;

#[tauri::command]
pub fn vm_info(state: State<AppState>) -> Result<VmInfo, AppError> {
    state
        .vm
        .info()
        .map_err(|e| AppError::Other(e))
}

#[tauri::command]
pub fn vm_provision(state: State<AppState>) -> Result<VmInfo, AppError> {
    state
        .vm
        .provision()
        .map_err(|e| AppError::Other(e))
}

#[tauri::command]
pub fn vm_start(state: State<AppState>) -> Result<VmInfo, AppError> {
    state.vm.start().map_err(|e| AppError::Other(e))
}

#[tauri::command]
pub fn vm_stop(state: State<AppState>) -> Result<VmInfo, AppError> {
    state.vm.stop().map_err(|e| AppError::Other(e))
}

#[tauri::command]
pub fn vm_restart(state: State<AppState>) -> Result<VmInfo, AppError> {
    state.vm.restart().map_err(|e| AppError::Other(e))
}

#[tauri::command]
pub fn vm_guest_health(state: State<AppState>) -> Result<bool, AppError> {
    state.vm.guest_health().map_err(|e| AppError::Other(e))
}

#[tauri::command]
pub fn vm_guest_request(
    state: State<AppState>,
    request: GuestRequest,
) -> Result<GuestResponse, AppError> {
    state.vm.guest_request(request).map_err(|e| AppError::Other(e))
}
