//! Build-time checks: hosted remote capability is narrow; legacy commands are denied.

use gptbot_lib::desktop_origin::{production_app_url, production_web_origin, PRODUCTION_WEB_ORIGIN};

#[test]
fn hosted_remote_capability_uses_exact_elsewhere_origin() {
    let raw =
        std::fs::read_to_string("capabilities/hosted-remote.json").expect("hosted-remote");
    let value: serde_json::Value = serde_json::from_str(&raw).expect("json");
    let urls = value["remote"]["urls"]
        .as_array()
        .expect("remote urls array");
    assert_eq!(urls.len(), 1);
    let pattern = urls[0].as_str().expect("url pattern");
    let expected = format!("{}/**", PRODUCTION_WEB_ORIGIN);
    assert_eq!(pattern, expected);
    assert!(!pattern.starts_with("https://*"));
}

#[test]
fn production_origin_matches_tauri_window_and_capability() {
    let conf =
        std::fs::read_to_string("tauri.conf.json").expect("tauri.conf.json");
    let value: serde_json::Value = serde_json::from_str(&conf).expect("json");
    let window_url = value["app"]["windows"][0]["url"]
        .as_str()
        .expect("main window url");
    assert_eq!(window_url, production_app_url());
    assert_eq!(production_web_origin(), PRODUCTION_WEB_ORIGIN);
}

#[test]
fn companion_permission_allowlist_is_narrow() {
    let raw =
        std::fs::read_to_string("permissions/companion-commands.toml").expect("permissions");
    assert!(raw.contains("get_this_mac_status"));
    assert!(raw.contains("set_this_mac_paused"));
    assert!(raw.contains("start_this_mac_pairing"));
    assert!(raw.contains("set_this_mac_onboarding_dismissed"));
    for denied in [
        "set_openai_api_key",
        "start_chat",
        "bootstrap_bots",
        "vm_provision",
    ] {
        assert!(raw.contains(denied), "missing deny entry for {denied}");
    }
}

#[test]
fn hosted_remote_includes_companion_and_deny_permissions() {
    let raw =
        std::fs::read_to_string("capabilities/hosted-remote.json").expect("hosted-remote");
    assert!(raw.contains("companion-commands"));
    assert!(raw.contains("deny-legacy-desktop-commands"));
}
