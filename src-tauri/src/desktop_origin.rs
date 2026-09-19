//! Production desktop webview origin (hosted Elsewhere www). Must match remote capabilities.

/// Single source of truth — must match `tauri.conf.json` main window URL and `hosted-remote.json`.
pub const PRODUCTION_WEB_ORIGIN: &str = "https://elsewhere-alpha-web.fly.dev";

pub fn production_web_origin() -> &'static str {
    PRODUCTION_WEB_ORIGIN
}

pub fn production_app_url() -> String {
    format!("{}/app", PRODUCTION_WEB_ORIGIN)
}

pub fn remote_capability_pattern() -> String {
    format!("{}/**", PRODUCTION_WEB_ORIGIN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_app_url_matches_tauri_window_path() {
        assert_eq!(
            production_app_url(),
            "https://elsewhere-alpha-web.fly.dev/app"
        );
    }
}

/// macOS computer name for This Mac display (never a secret).
pub fn local_mac_device_name() -> String {
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
