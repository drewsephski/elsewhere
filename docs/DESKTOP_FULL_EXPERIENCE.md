# Elsewhere desktop (full experience)

## Production architecture

```text
Elsewhere.app (Tauri host)
    ↓ main webview URL
https://elsewhere-alpha-web.fly.dev/app
    ↓ same origin
Better Auth (/api/auth) + cloud BFF (/api/cloud)
    ↓ narrow IPC (remote capability)
get_this_mac_status · set_this_mac_paused · start_this_mac_pairing · set_this_mac_onboarding_skipped
```

- **Production loads hosted Elsewhere first-party** — session cookies and Better Auth behave like the browser; no cross-origin cookie transport from a bundled Vite origin.
- **Tauri supplies** Keychain credential, host link, tray/lifecycle, and the narrow companion bridge only.
- **Bundled `CloudShell` (localhost:1420)** remains for local development: shared `apps/www` UI via aliases, with `workspace-http-origin` pointing API calls at local Next or configured hosted origin when the webview is not already on Elsewhere www.

### Trust boundary

- Remote capability URL pattern: `https://elsewhere-alpha-web.fly.dev/**` only (no `https://*`).
- `ELSEWHERE_DESKTOP_WEBVIEW_URL` is removed; production origin is compile-time config (`desktop_origin.rs` / `tauri.conf.json` window `url`).
- Legacy bot/chat/VM invoke handlers may remain in the binary but are **denied** for remote pages via `deny-legacy-desktop-commands`.
- Secrets, pairing material, and filesystem access stay in Rust; hosted React never receives tokens from Keychain.

## Development

```text
localhost:1420 (Vite CloudShell)
  → shared apps/www components
  → NEXT_PUBLIC_ELSEWHERE_WEB_ORIGIN or local Next proxy for /api/*
```

## Desktop UX

- **Sign-in**: hosted auth UI with desktop-only hero when `isTauriRuntime()`.
- **First-run This Mac**: full-screen overlay (“Use this Mac”), not the Computers admin grid.
- **Not now**: persisted in local SQLite (`this_mac_onboarding_skipped`); no repeat nag; users with existing Bots are not interrupted.
- **Quick start / New bot**: prefers This Mac only when companion phase is **live**; shows “Runs on · This Mac” when applicable.
- **Cmd+,** Settings · **Cmd+N** New Bot (Tauri only).
- **Tray**: Open · status line · Pause/Resume · Quit (labels reflect pause/live/connecting).
- **Notifications**: Tauri notification plugin for approval backlog (permission requested when needed).

## Manual Mac .app checklist

See PR description / product brief for the full 22-step acceptance list (build without local Next, hosted sign-in, onboarding, pairing, hidden-window host link, pause/resume, native approval notification, quit).

## CI notes

- `src-tauri/tests/companion_acl.rs` asserts capability shape and permission allow/deny lists.
- Linux agents do not produce a signed `.app`; run `pnpm tauri build` on macOS for packaging smoke.
