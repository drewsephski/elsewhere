#[cfg(target_os = "macos")]
pub mod agent;
mod commands;
mod db;
mod error;
mod models;
mod openai;
mod secrets;
mod state;
#[cfg(target_os = "macos")]
pub mod vm;

use db::Database;
use secrets::KeychainSecretStore;
use state::AppState;
use std::sync::Arc;
use tauri::Manager;
use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("elsewhere=info".parse().unwrap())
                .add_directive("gptbot=info".parse().unwrap()),
        )
        .try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| -> Box<dyn std::error::Error> { Box::new(e) })?;
            let db_path = data_dir.join("gptbot.sqlite3");
            let database = Database::open(&db_path)?;
            let secrets: Arc<dyn secrets::SecretStore> = Arc::new(KeychainSecretStore);

            #[cfg(target_os = "macos")]
            let vm = Arc::new(crate::vm::VirtualMachineManager::new(&data_dir));

            app.manage(AppState {
                db: Arc::new(parking_lot::Mutex::new(database)),
                secrets,
                active_streams: parking_lot::Mutex::new(std::collections::HashMap::new()),
                #[cfg(target_os = "macos")]
                vm,
            });

            tracing::info!(path = %db_path.display(), "database initialized");

            if let Ok(external_url) = std::env::var("ELSEWHERE_DESKTOP_WEBVIEW_URL") {
                let trimmed = external_url.trim();
                if !trimmed.is_empty() {
                    if let Some(window) = app.get_webview_window("main") {
                        match trimmed.parse::<url::Url>() {
                            Ok(parsed) => {
                                if let Err(error) = window.navigate(parsed) {
                                    tracing::warn!(
                                        %error,
                                        url = %trimmed,
                                        "ELSEWHERE_DESKTOP_WEBVIEW_URL navigation failed"
                                    );
                                } else {
                                    tracing::info!(url = %trimmed, "desktop webview navigated to hosted workspace");
                                }
                            }
                            Err(error) => {
                                tracing::warn!(
                                    %error,
                                    url = %trimmed,
                                    "invalid ELSEWHERE_DESKTOP_WEBVIEW_URL"
                                );
                            }
                        }
                    }
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap_bots,
            commands::list_bots,
            commands::get_bot,
            commands::create_bot,
            commands::update_bot,
            commands::archive_bot,
            commands::delete_bot,
            commands::list_conversations,
            commands::get_or_create_conversation,
            commands::list_messages,
            commands::get_api_key_status,
            commands::set_openai_api_key,
            commands::clear_openai_api_key,
            commands::list_openai_models,
            commands::start_chat,
            commands::cancel_chat,
            #[cfg(target_os = "macos")]
            commands::start_agent_chat,
            #[cfg(target_os = "macos")]
            commands::vm_info,
            #[cfg(target_os = "macos")]
            commands::vm_provision,
            #[cfg(target_os = "macos")]
            commands::vm_start,
            #[cfg(target_os = "macos")]
            commands::vm_wait_guest,
            #[cfg(target_os = "macos")]
            commands::vm_stop,
            #[cfg(target_os = "macos")]
            commands::vm_restart,
            #[cfg(target_os = "macos")]
            commands::vm_guest_health,
            #[cfg(target_os = "macos")]
            commands::vm_guest_request,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
