use crate::error::AppError;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElsewherePairingStatus {
    pub connected: bool,
    pub pairing: bool,
    pub node_id: Option<String>,
    pub computer_id: Option<String>,
    pub user_code: Option<String>,
    pub live_session: bool,
    pub reauth_required: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartPairingResponse {
    pairing_id: String,
    pairing_secret: String,
    user_code: String,
    expires_at: String,
    verification_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExchangePairingResponse {
    node_id: Option<String>,
    computer_id: Option<String>,
    credential: Option<String>,
    display_name: Option<String>,
    status: Option<String>,
    error: Option<String>,
}

fn elsewhere_web_origin() -> String {
    std::env::var("ELSEWHERE_WEB_ORIGIN")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            if cfg!(debug_assertions) {
                "http://127.0.0.1:3000".into()
            } else {
                "https://elsewhere-alpha-web.fly.dev".into()
            }
        })
}

fn local_device_name() -> String {
    std::process::Command::new("scutil")
        .args(["--get", "ComputerName"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "This Mac".into())
}

#[tauri::command]
pub fn get_elsewhere_pairing_status(
    state: State<AppState>,
) -> Result<ElsewherePairingStatus, AppError> {
    let identity = {
        let db = state.db.lock();
        db.elsewhere_pairing_identity()?
    };
    let credential = state.secrets.get_elsewhere_device_credential()?;
    let connected = identity.is_some() && credential.is_some();
    let (live_session, reauth_required) = host_link_flags(&state);
    Ok(ElsewherePairingStatus {
        connected,
        pairing: false,
        node_id: identity.as_ref().map(|(node_id, _)| node_id.clone()),
        computer_id: identity
            .as_ref()
            .map(|(_, computer_id)| computer_id.clone()),
        user_code: None,
        live_session,
        reauth_required,
    })
}

#[tauri::command]
pub async fn start_elsewhere_pairing(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ElsewherePairingStatus, AppError> {
    let installation_id = {
        let db = state.db.lock();
        db.installation_id()?
    };
    let device_name = local_device_name();
    let origin = elsewhere_web_origin().trim_end_matches('/').to_string();
    let client = reqwest::Client::new();
    let start = client
        .post(format!("{origin}/api/local-mac/pairings"))
        .json(&serde_json::json!({
            "deviceName": device_name,
            "installationId": installation_id,
        }))
        .send()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;
    if !start.status().is_success() {
        let body = start.text().await.unwrap_or_default();
        return Err(AppError::Provider(format!(
            "Could not start pairing: {body}"
        )));
    }
    let started: StartPairingResponse = start
        .json()
        .await
        .map_err(|e| AppError::Provider(e.to_string()))?;

    app.opener()
        .open_url(&started.verification_url, None::<&str>)
        .map_err(|e| AppError::Other(e.to_string()))?;

    let deadline = chrono::DateTime::parse_from_rfc3339(&started.expires_at)
        .ok()
        .map(|value| value.with_timezone(&chrono::Utc))
        .unwrap_or_else(|| chrono::Utc::now() + chrono::Duration::minutes(10));

    loop {
        if chrono::Utc::now() >= deadline {
            return Err(AppError::Validation(
                "Pairing expired. Start Connect to Elsewhere again.".into(),
            ));
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let exchange = client
            .post(format!(
                "{origin}/api/local-mac/pairings/{}/exchange",
                started.pairing_id
            ))
            .json(&serde_json::json!({
                "pairingSecret": started.pairing_secret,
            }))
            .send()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?;
        let status = exchange.status();
        let body: ExchangePairingResponse = exchange
            .json()
            .await
            .map_err(|e| AppError::Provider(e.to_string()))?;
        if status.as_u16() == 202 || body.status.as_deref() == Some("pending") {
            continue;
        }
        if !status.is_success() {
            return Err(AppError::Provider(
                body.error
                    .unwrap_or_else(|| "Pairing was not approved".into()),
            ));
        }
        let credential = body.credential.ok_or_else(|| {
            AppError::Provider("pairing did not return a device credential".into())
        })?;
        let node_id = body
            .node_id
            .ok_or_else(|| AppError::Provider("pairing did not return a node id".into()))?;
        let computer_id = body
            .computer_id
            .ok_or_else(|| AppError::Provider("pairing did not return a computer id".into()))?;
        state.secrets.set_elsewhere_device_credential(&credential)?;
        {
            let db = state.db.lock();
            db.set_elsewhere_pairing_identity(&node_id, &computer_id)?;
        }
        let _ = body.display_name;
        #[cfg(target_os = "macos")]
        state.host_link.notify_credential_ready();
        let (live_session, reauth_required) = host_link_flags(&state);
        return Ok(ElsewherePairingStatus {
            connected: true,
            pairing: false,
            node_id: Some(node_id),
            computer_id: Some(computer_id),
            user_code: Some(started.user_code),
            live_session,
            reauth_required,
        });
    }
}

fn host_link_flags(state: &AppState) -> (bool, bool) {
    #[cfg(target_os = "macos")]
    {
        match state.host_link.state() {
            crate::host_link::HostLinkState::Connected => (true, false),
            crate::host_link::HostLinkState::ReauthRequired => (false, true),
            _ => (false, false),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = state;
        (false, false)
    }
}
