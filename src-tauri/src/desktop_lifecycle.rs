use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, RunEvent, WindowEvent,
};

use crate::commands::{this_mac_status_snapshot, ThisMacPhase};
use crate::state::AppState;

fn tray_status_label(phase: ThisMacPhase, paused: bool) -> String {
    if paused {
        return "This Mac: Paused".to_string();
    }
    match phase {
        ThisMacPhase::Live => "This Mac: Live".to_string(),
        ThisMacPhase::Connecting | ThisMacPhase::Connected | ThisMacPhase::Reconnecting => {
            "This Mac: Connecting".to_string()
        }
        ThisMacPhase::Paused => "This Mac: Paused".to_string(),
        ThisMacPhase::Offline | ThisMacPhase::Reauth | ThisMacPhase::Disconnected => {
            "This Mac: Offline".to_string()
        }
    }
}

fn build_tray_menu(app: &tauri::AppHandle) -> Result<Menu<tauri::Wry>, tauri::Error> {
    let (phase, paused) = if let Some(state) = app.try_state::<AppState>() {
        match this_mac_status_snapshot(&state) {
            Ok(status) => (status.phase, status.paused),
            Err(_) => (ThisMacPhase::Disconnected, false),
        }
    } else {
        (ThisMacPhase::Disconnected, false)
    };

    let open_i = MenuItem::with_id(app, "open", "Open Elsewhere", true, None::<&str>)?;
    let status_i = MenuItem::with_id(
        app,
        "status",
        &tray_status_label(phase, paused),
        false,
        None::<&str>,
    )?;
    let pause_label = if paused {
        "Resume This Mac"
    } else {
        "Pause This Mac"
    };
    let pause_i = MenuItem::with_id(app, "pause", pause_label, true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit Elsewhere", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    Menu::with_items(app, &[&open_i, &status_i, &separator, &pause_i, &quit_i])
}

pub fn notify_tray_status_changed(app: &tauri::AppHandle) {
    if let Some(tray) = app.tray_by_id("elsewhere-main") {
        if let Ok(menu) = build_tray_menu(app) {
            let _ = tray.set_menu(Some(menu));
            if let Some(state) = app.try_state::<AppState>() {
                if let Ok(status) = this_mac_status_snapshot(&state) {
                    let _ = tray.set_tooltip(Some(&tray_status_label(status.phase, status.paused)));
                }
            }
        }
    }
}

pub fn attach(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(window) = app.get_webview_window("main") {
        let hide_window = window.clone();
        window.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = hide_window.hide();
            }
        });
    }

    let menu = build_tray_menu(app.handle())?;

    let _tray = TrayIconBuilder::with_id("elsewhere-main")
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
                    if let Some(state) = app.try_state::<AppState>() {
                        let paused = {
                            let db = state.db.lock();
                            !db.this_mac_paused().unwrap_or(false)
                        };
                        {
                            let db = state.db.lock();
                            let _ = db.set_this_mac_paused(paused);
                        }
                        #[cfg(target_os = "macos")]
                        {
                            if paused {
                                state.host_link.break_active_session();
                            }
                            state.host_link.notify_credential_ready();
                        }
                    }
                    notify_tray_status_changed(app);
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

    notify_tray_status_changed(app.handle());

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
