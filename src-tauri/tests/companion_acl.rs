//! Build-time checks: hosted remote capability is narrow; legacy commands are denied.

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
    assert!(
        pattern.starts_with("https://elsewhere-alpha-web.fly.dev/"),
        "unexpected pattern: {pattern}"
    );
    assert!(!pattern.contains('*') || pattern.ends_with("/**"));
    assert!(!pattern.starts_with("https://*"));
}

#[test]
fn companion_permission_allowlist_is_narrow() {
    let raw =
        std::fs::read_to_string("permissions/companion-commands.toml").expect("permissions");
    assert!(raw.contains("get_this_mac_status"));
    assert!(raw.contains("set_this_mac_paused"));
    assert!(raw.contains("start_this_mac_pairing"));
    assert!(raw.contains("set_this_mac_onboarding_skipped"));
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
