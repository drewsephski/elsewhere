mod commands;
mod db;
mod error;
mod models;
mod openai;
mod secrets;
mod state;

use db::Database;
use secrets::KeychainSecretStore;
use state::AppState;
use std::sync::Arc;
use tauri::Manager;
use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("gptbot=info".parse().unwrap()))
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

            app.manage(AppState {
                db: parking_lot::Mutex::new(database),
                secrets,
                active_streams: parking_lot::Mutex::new(std::collections::HashMap::new()),
            });

            tracing::info!(path = %db_path.display(), "database initialized");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
