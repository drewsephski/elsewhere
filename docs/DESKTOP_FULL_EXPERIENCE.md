# Elsewhere desktop (full experience draft)

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│  Tauri .app (Rust host)                                      │
│  · Keychain device credential                                │
│  · host_link outbound WS → cloud-host (This Mac RPC)         │
│  · Narrow IPC: get/set This Mac status + pairing only        │
│  · Tray + hide-on-close lifecycle                          │
└───────────────────────────┬─────────────────────────────────┘
                            │ WebView
┌───────────────────────────▼─────────────────────────────────┐
│  Vite bundle (`src/main.tsx` → `CloudShell`)                  │
│  Same React tree as `apps/www` via `@` aliases                │
│  `workspace-http-origin` → hosted `/api/auth` + `/api/cloud`  │
└───────────────────────────┬─────────────────────────────────┘
                            │ HTTPS (session cookies)
┌───────────────────────────▼─────────────────────────────────┐
│  Hosted Elsewhere www (Next.js BFF) — not bundled in .app     │
└─────────────────────────────────────────────────────────────┘
```

- **No Next.js inside the .app** — production ships `vite build` static assets only.
- **Browser unchanged** — relative `/api/cloud` BFF on the web; desktop uses the same client modules with an absolute hosted origin when `isTauriRuntime()`.
- **Compromised webview** cannot reach legacy bot/chat SQLite commands from product UI; companion IPC is limited to pairing/pause/status.

## Manual Mac .app checklist (when CI cannot build Tauri)

1. `pnpm build` with `NEXT_PUBLIC_BETTER_AUTH_URL` / `NEXT_PUBLIC_ELSEWHERE_WEB_ORIGIN` set to hosted www.
2. `pnpm tauri build` on macOS with signing entitlements.
3. Launch .app → sign in → lands in `/app` workspace (not legacy OpenAI settings).
4. **Use this Mac** on Computers → pairing completes → titlebar shows Live.
5. **Pause This Mac** → host link disconnects; resume reconnects.
6. Cmd+W hides window; Dock icon reopens; tray Quit exits.
7. Approval / attention notifications (after granting permission).
8. Navigate connectors, channels, routine detail — same as web.

## Limitations (draft PR)

- Tray “Pause” toggles without reflecting live label until next status poll.
- Native notifications use the Web Notification API (not macOS notification center extensions).
- Linux CI does not produce a signed `.app`; Rust unit tests cover host_link pause only.
