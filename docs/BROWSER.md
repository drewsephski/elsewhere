# Browser automation (agent computer)

Headless web browsing runs **inside the Fly Sprite guest**, not on the cloud-host runner. Luna calls the same MCP / Responses tool surface as workspace tools; `dispatch_tool_with_gate` enforces approvals before any browser side effect.

## Tools

| Tool | Approval | Purpose |
| --- | --- | --- |
| `browser_navigate` | Mutation | Open an http(s) URL |
| `browser_snapshot` | Read | Page URL, title, interactive refs (`e1`, `e2`, …) |
| `browser_click` | Mutation | Click a ref from snapshot |
| `browser_type` | Mutation | Fill an input ref; optional Enter |
| `browser_screenshot` | Read | PNG under `/workspace/…` |
| `browser_download` | Mutation | Fetch a URL into a workspace file |

Refs are valid until the next snapshot on that page session. Profile data persists under `/workspace/.elsewhere/browser/profile`.

## Guest implementation

- Driver: `crates/sprite-computer/guest/browser-cli.mjs` (Playwright Core + downloaded Chromium)
- First boot (per Sprite, persisted under `/workspace/.elsewhere/browser/`):
  - Portable Node tarball (when the guest has no package manager)
  - `npm install playwright-core`
  - `playwright install chromium` + `install-deps` (Debian-based Sprites)
- Session URL is stored in `session.json` so separate tool calls share the active page.
- Invocation: write JSON request → `node …/cli.mjs --request …` via Sprite exec API (120s timeout; bootstrap up to 300s).

## Network policy

When `ELSEWHERE_BROWSER_ENABLED` is true (default), new and existing Sprites receive **allow \*** egress so Chromium can reach the public web. Shell `workspace_exec` on that Sprite also has network; keep approvals on mutations.

Set `ELSEWHERE_BROWSER_ENABLED=false` to restore default-deny egress and disable browser bootstrap.

## Observability

Durable `tool_call` / `tool_result` events use the same pipeline as workspace tools. The workspace Computer panel and work timeline map browser tools to human labels via `apps/www/lib/work-events.ts`.

## Suggested E2E assignment

1. `browser_navigate` to a few research URLs (approve each navigation if required).
2. `browser_snapshot` / `browser_click` / `browser_type` as needed.
3. `browser_download` or `browser_screenshot` into `/workspace/results/{run-id}/`.
4. `workspace_write` a short summary citing sources; confirm artifact download from Results.

Live smoke (optional):

```bash
ELSEWHERE_BROWSER_SMOKE=1 SPRITE_TOKEN=… ELSEWHERE_TEST_SPRITE=… \
  cargo run -p sprite-computer --example smoke
```

## Code map

- Tool catalog & dispatch: `crates/agent-core/src/tool_catalog.rs`, `browser_tools.rs`, `tools.rs`, `approval.rs`
- Sprite adapter: `crates/sprite-computer/src/browser.rs`, `computer.rs`, `policy.rs`
- MCP mirror: `crates/computer-mcp/src/tools.rs`
- Codex allowlist: `crates/codex-provider/src/protocol/thread.rs` (`ALL_COMPUTER_TOOL_NAMES`)
- Hosted config: `ELSEWHERE_BROWSER_ENABLED` in `services/cloud-host/src/config.rs`, network policy in `runner.rs`
