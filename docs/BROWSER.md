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

## Network and safety

- http(s) only; blocks loopback, RFC1918, link-local, CGNAT (`100.64.0.0/10`), metadata hostnames, and private IPv6 at validation (Rust + guest) and on page subrequests (daemon route guard)
- Hostnames are **DNS-resolved** before navigation/download; any resolved private/link-local/metadata address is rejected (SSRF hardening)
- `browser_download` follows redirects manually with per-hop URL validation; size and timeout caps apply to the final body (not only route interception)
- Caps: snapshot element count, download/screenshot bytes, type text length, navigation/action timeouts (see `browser-common.mjs`)

## Observability

Durable `tool_call` / `tool_result` events use the same pipeline as workspace tools. The workspace Computer panel maps browser tools via `apps/www/lib/work-events.ts`.

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
- Hosted config: `ELSEWHERE_BROWSER_ENABLED` in `services/cloud-host/src/config.rs`
