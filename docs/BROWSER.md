# Browser automation (agent computer)

Headless web browsing runs **inside the Fly Sprite guest**, not on the cloud-host runner. Luna calls the same MCP / Responses tool surface as workspace tools; `dispatch_tool_with_gate` enforces approvals before any browser side effect.

## Tools

| Tool | Approval | Purpose |
| --- | --- | --- |
| `browser_navigate` | Mutation | Open an http(s) URL |
| `browser_snapshot` | Read | Page URL, title, interactive refs (`e1`, `e2`, …) |
| `browser_click` | Mutation | Click a ref from snapshot |
| `browser_type` | Mutation | Fill an input ref; optional Enter |
| `browser_screenshot` | Mutation | PNG under `/workspace/…` |
| `browser_download` | Mutation | Fetch a URL into a workspace file |

Refs stay valid while the browser daemon keeps the same page open. A new `browser_snapshot` re-labels elements but the underlying session (cookies, `localStorage`, URL) persists in the daemon.

## Guest implementation

- **Daemon** (persistent Playwright context): `crates/sprite-computer/guest/browser-daemon.mjs`
- **Client** (one request per tool call over loopback Unix socket): `browser-client.mjs`
- **Shared logic** (URL policy, caps): `browser-common.mjs`
- **Runtime root** (outside workspace): `/var/elsewhere/browser` — Chromium profile, npm/playwright install, socket at `daemon.sock`
- **User artifacts only** under `/workspace` (screenshots, downloads, summaries)
- Bootstrap pins `playwright-core@1.49.1`, Node `v20.18.0`, and a `bootstrap-version` file for deterministic upgrades
- Hosted runner keeps Sprite egress **default-deny**; `SpriteComputer` temporarily sets allow-all only during bootstrap and each browser tool call, then **restores and verifies** default-deny via the Sprites network policy API (failure fails closed and surfaces a guest/readiness error)
- A per-Sprite **execution gate** mutex serializes `workspace_exec` and all browser/bootstrap work so shell exec cannot run while egress is open
- The browser daemon **serializes** RPCs on a single queue so concurrent MCP browser calls cannot race the same page/refs

`ELSEWHERE_BROWSER_ENABLED=false` disables browser bootstrap and tool dispatch on the host.

## Sign-in persistence (host-only browser profiles)

Each computer's Chromium user data (cookies, `localStorage`, saved sign-ins) is bundled from the Sprite into a **host-only** directory on the runner and restored on the next session, so sign-ins survive Sprite restarts and runner deploys. `services/cloud-host/src/browser_profile.rs` maps `computer_id → profile UUID` in `browser_profiles` and stores bytes under `ELSEWHERE_BROWSER_PROFILES_DIR/<profile-uuid>/` (mode 0700). The owner-only **Reset browser sign-in** endpoint rotates the UUID, deletes the host bytes, and clears the guest profile.

| Environment | `ELSEWHERE_BROWSER_PROFILES_DIR` | Notes |
| --- | --- | --- |
| Fly alpha runner | `/var/lib/elsewhere/browser-profiles` | Set in `infra/fly/runner.toml` (source of truth) and as an image default in `Dockerfile.runner`. Lives on the same encrypted volume as `codex-profiles`; `runner-entrypoint.sh` refuses to start unless the env matches this path, then creates it `0700 elsewhere:elsewhere` and verifies it is writable **before** dropping to UID 10001. |
| Other hosted / self-hosted | absolute path on a persistent volume | Must be set explicitly. The service user must be able to write it after privilege drop; the runner cannot create the root itself. Never under `/workspace`, the image rootfs, or a temp dir. |
| Local hybrid dev | unset → `<repo>/.data/browser-profiles` | Convenience default only. Non-hybrid modes that fall back to the working directory log a startup warning. |

Symptom of a misconfigured or missing root: `Browser profile storage unavailable: Permission denied (os error 13)` when a computer first uses the browser (the host tried to create `<cwd>/.data/browser-profiles` as the unprivileged service user). Fix the env/volume; do not widen permissions on the container filesystem.

## Network and safety

- http(s) only; blocks loopback, RFC1918, link-local, CGNAT (`100.64.0.0/10`), metadata hostnames, and private IPv6 at validation (Rust + guest) and on page subrequests (daemon route guard)
- Hostnames are **DNS-resolved** before navigation/download; any resolved private/link-local/metadata address is rejected (SSRF hardening)
- `browser_download` follows redirects manually with per-hop URL validation; size and timeout caps apply to the final body (not only route interception)
- Caps: snapshot element count, download/screenshot bytes, type text length, navigation/action timeouts (see `browser-common.mjs`)

## Observability

Durable `tool_call` / `tool_result` events use the same pipeline as workspace tools. The workspace Computer panel maps browser tools via `apps/www/lib/work-events.ts`.

### Human takeover (owner control lease)

While a run is active, the owner can **Take control** from the live browser preview. Control state is stored in `computer_control_leases` (`bot` vs `human`) with heartbeat-based stale recovery (default 120s). Bot browser **mutations** wait until control returns; `browser_snapshot` and preview reads continue. Owner-only input endpoints: `POST …/browser-control/take|return`, `GET …/browser-control`, and `POST …/browser/{navigate,click,scroll,type,press-key}` (same persistent Sprite session — no second browser). The workspace preview is a remote surface: take control, then click, type, and scroll the live view directly. Return to bot restores agent-driven control.

While a run is active, the Computer rail polls `GET /v1/computers/{id}/browser-preview` every ~2s (about 800ms while you have control). The host calls the guest daemon `preview` action (viewport JPEG, base64, no workspace write, no tool approval). Preview frames skip temporary egress widening; only `navigate` and `download` open the network gate.

The workspace UI shares one poll via `BrowserPreviewProvider`: embedded preview in the Computer rail, a floating picture-in-picture over the chat (manual **Pop out** or auto when you scroll), expand dialog, and native **Fullscreen** (`requestFullscreen`). The work detail page (`/app/work/[id]`) uses the same preview while a run is active.

## Verified smoke

With Fly credentials and `ELSEWHERE_BROWSER_SMOKE=1`:

```bash
cargo run -p sprite-computer --example smoke
```

Proves workspace I/O plus, when browser smoke is on:

1. `browser_navigate` → `browser_snapshot` → `browser_click` (ref from snapshot) → snapshot on a new URL
2. `browser_navigate` → `browser_snapshot` → `browser_type` → later snapshot still shows typed text

## Hosted Luna acceptance

After CI passes, run the hosted Luna acceptance prompt (three public sites, snapshot refs, form typing, screenshot + markdown report under `/workspace/results/<RUN_ID>/`). Use browser tools only; no private/local targets or authenticated accounts.

## Code map

- Tool catalog & dispatch: `crates/agent-core/src/tool_catalog.rs`, `browser_tools.rs`, `public_http_url.rs`, `tools.rs`, `approval.rs`
- Sprite adapter: `crates/sprite-computer/src/browser.rs`, `computer.rs`, `policy.rs`
- MCP mirror: `crates/computer-mcp/src/tools.rs`
- Hosted config: `ELSEWHERE_BROWSER_ENABLED`, `ELSEWHERE_BROWSER_PROFILES_DIR` in `services/cloud-host/src/config.rs`; host profile storage in `browser_profile.rs`
- Fly runner volume layout: `infra/fly/runner.toml`, `infra/fly/runner-entrypoint.sh`
