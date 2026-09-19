use crate::commands::elsewhere::{start_elsewhere_pairing, ElsewherePairingStatus};
use crate::desktop_origin::local_mac_device_name;
use crate::error::AppError;
use crate::state::AppState;
use serde::Serialize;
use tauri::{AppHandle, State};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ThisMacPhase {
    Disconnected,
    Connecting,
    Connected,
    Live,
    Offline,
    Reconnecting,
    Reauth,
    Paused,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThisMacStatus {
    pub phase: ThisMacPhase,
    pub paired: bool,
    pub pairing_in_progress: bool,
    pub paused: bool,
    pub onboarding_skipped: bool,
    pub device_name: String,
    pub node_id: Option<String>,
    pub computer_id: Option<String>,
    pub user_code: Option<String>,
}

fn read_paused(state: &AppState) -> Result<bool, AppError> {
    let db = state.db.lock();
    Ok(db.this_mac_paused()?)
}

fn read_onboarding_skipped(state: &AppState) -> Result<bool, AppError> {
    let db = state.db.lock();
    Ok(db.this_mac_onboarding_skipped()?)
}

fn base_status(
    pairing: &ElsewherePairingStatus,
    phase: ThisMacPhase,
    paired: bool,
    pairing_in_progress: bool,
    paused: bool,
    onboarding_skipped: bool,
) -> ThisMacStatus {
    ThisMacStatus {
        phase,
        paired,
        pairing_in_progress,
        paused,
        onboarding_skipped,
        device_name: local_mac_device_name(),
        node_id: pairing.node_id.clone(),
        computer_id: pairing.computer_id.clone(),
        user_code: pairing.user_code.clone(),
    }
}

pub fn status_from_parts(
    pairing: &ElsewherePairingStatus,
    paused: bool,
    reconnecting: bool,
    onboarding_skipped: bool,
) -> ThisMacStatus {
    let paired = pairing.connected;
    if paused {
        return base_status(
            pairing,
            ThisMacPhase::Paused,
            paired,
            pairing.pairing,
            true,
            onboarding_skipped,
        );
    }
    if pairing.reauth_required {
        return base_status(
            pairing,
            ThisMacPhase::Reauth,
            paired,
            pairing.pairing,
            false,
            onboarding_skipped,
        );
    }
    if pairing.pairing {
        return base_status(
            pairing,
            ThisMacPhase::Connecting,
            paired,
            true,
            false,
            onboarding_skipped,
        );
    }
    if pairing.live_session {
        return base_status(
            pairing,
            ThisMacPhase::Live,
            paired,
            false,
            false,
            onboarding_skipped,
        );
    }
    if pairing.connected {
        let phase = if reconnecting {
            ThisMacPhase::Reconnecting
        } else {
            ThisMacPhase::Connected
        };
        return base_status(pairing, phase, true, false, false, onboarding_skipped);
    }
    let phase = if reconnecting {
        ThisMacPhase::Reconnecting
    } else if paired {
        ThisMacPhase::Offline
    } else {
        ThisMacPhase::Disconnected
    };
    base_status(pairing, phase, paired, false, false, onboarding_skipped)
}

pub fn this_mac_status_snapshot(state: &AppState) -> Result<ThisMacStatus, AppError> {
    let pairing = crate::commands::elsewhere::pairing_status_snapshot(state)?;
    let paused = read_paused(state)?;
    let reconnecting = host_link_reconnecting(state);
    let onboarding_skipped = read_onboarding_skipped(state)?;
    Ok(status_from_parts(&pairing, paused, reconnecting, onboarding_skipped))
}

#[tauri::command]
pub fn get_this_mac_status(state: State<AppState>) -> Result<ThisMacStatus, AppError> {
    this_mac_status_snapshot(&state)
}

#[tauri::command]
pub async fn set_this_mac_paused(
    app: AppHandle,
    state: State<'_, AppState>,
    paused: bool,
) -> Result<ThisMacStatus, AppError> {
    let app_handle = app.clone();
    {
        let db = state.db.lock();
        db.set_this_mac_paused(paused)?;
    }
    #[cfg(target_os = "macos")]
    {
        if paused {
            state.host_link.break_active_session();
        }
        state.host_link.notify_credential_ready();
    }
    crate::desktop_lifecycle::notify_tray_status_changed(&app_handle);
    get_this_mac_status(state)
}

#[tauri::command]
pub async fn start_this_mac_pairing(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ThisMacStatus, AppError> {
    let paused = read_paused(&state)?;
    let onboarding_skipped = read_onboarding_skipped(&state)?;
    let pairing = start_elsewhere_pairing(app, state).await?;
    Ok(status_from_parts(&pairing, paused, false, onboarding_skipped))
}

#[tauri::command]
pub fn set_this_mac_onboarding_skipped(
    state: State<AppState>,
    skipped: bool,
) -> Result<ThisMacStatus, AppError> {
    {
        let db = state.db.lock();
        db.set_this_mac_onboarding_skipped(skipped)?;
    }
    get_this_mac_status(state)
}

fn host_link_reconnecting(state: &AppState) -> bool {
    #[cfg(target_os = "macos")]
    {
        return state.host_link.reconnecting();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = state;
        false
    }
}
