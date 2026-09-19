//! Production desktop webview origin (hosted Elsewhere www). Must match remote capabilities.

pub const DEFAULT_PRODUCTION_WEB_ORIGIN: &str = "https://elsewhere-alpha-web.fly.dev";

pub fn production_web_origin() -> String {
    std::env::var("ELSEWHERE_DESKTOP_WEB_ORIGIN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_PRODUCTION_WEB_ORIGIN.to_string())
        .trim_end_matches('/')
        .to_string()
}

pub fn production_app_url() -> String {
    format!("{}/app", production_web_origin())
}

pub fn remote_capability_pattern() -> String {
    format!("{}/**", production_web_origin())
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
