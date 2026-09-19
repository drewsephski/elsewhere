use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, RunEvent, WindowEvent,
};

pub fn attach(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(window) = app.get_webview_window("main") {
        let window = window.clone();
        window.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        });
    }

    let open_i = MenuItem::with_id(app, "open", "Open Elsewhere", true, None::<&str>)?;
    let pause_i = MenuItem::with_id(app, "pause", "Pause This Mac", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit Elsewhere", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_i, &pause_i, &quit_i])?;

    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("Elsewhere")
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "open" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "pause" => {
                    if let Some(state) = app.try_state::<crate::state::AppState>() {
                        let paused = {
                            let db = state.db.lock();
                            !db.this_mac_paused().unwrap_or(false)
                        };
                        let db = state.db.lock();
                        let _ = db.set_this_mac_paused(paused);
                        #[cfg(target_os = "macos")]
                        state.host_link.notify_credential_ready();
                    }
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

pub fn handle_run_event(app: &tauri::AppHandle, event: &RunEvent) {
    if let RunEvent::Reopen { has_visible_windows, .. } = event {
        if !has_visible_windows {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    }
}
