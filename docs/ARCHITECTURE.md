# Elsewhere — Architecture

Current cloud product: [authenticated pairing](PHASE_3C1.md), [background work](BACKGROUND_WORK.md), and [product progress](PRODUCT_PROGRESS.md). The phase diagrams below preserve the original local-runtime design; Codex execution, ownership, and approvals are now implemented.

## Repository layout

| Package | Path | Role |
|---------|------|------|
| `@elsewhere/desktop` | repo root (`src/`, `src-tauri/`) | Tauri + Vite chat shell |
| `@elsewhere/www` | `apps/www/` | Next.js marketing site |
| `@elsewhere/brand` | `packages/brand/` | Shared product copy and URLs |
| `agent-core` | `crates/agent-core/` | Host-independent Luna agent loop |
| `sprite-computer` | `crates/sprite-computer/` | Fly Sprites `AgentComputer` adapter |

Desktop and web are separate bundles. Run `pnpm dev:www` for marketing and `pnpm tauri dev` for the native app.

## Runtime boundary (portable core)

```text
RunEngine (Phase 3B.2+)
 ├── ResponsesRunEngine   — API key + agent-core::run_agent_loop (today)
 └── CodexRunEngine       — ChatGPT subscription via Codex app-server (planned)

ResponsesRunEngine:
 ├── ResponsesModel      (OpenAI Responses API — host-provided)
 ├── RunStore            (SQLite today, Postgres in cloud-host)
 ├── EventSink           (Tauri events today, SSE in cloud-host)
 └── AgentComputer (async)
      ├── LocalMacComputer   (desktop — VZ + guest-agent)
      └── SpriteComputer     (Fly Sprites REST — `crates/sprite-computer`)
```

See `docs/PHASE_3B2_CODEX_PROVIDER.md` for the Codex subscription path (preferred default when ChatGPT-authenticated Codex is available).

The core loop does **not** depend on Tauri, macOS, SQLite, or Virtualization.framework. The desktop app wires concrete adapters in `src-tauri/src/agent/`.

### Luna default

Authoritative default model: `gpt-5.6-luna` (`agent_core::DEFAULT_MODEL`). New bots and demos use Luna; if the account does not list Luna, the UI should surface that — not silently pick another model.

### Execution configuration

Bots store `computer_enabled` (not bot name):

- `false` — streaming chat completions only (e.g. demo Scout)
- `true` — Luna Responses tool loop + `LocalMacComputer`

## Process boundary (desktop)

- **Tauri webview**: React UI — `src/surfaces/desktop/`.
- **Rust host**: SQLite, Keychain, OpenAI HTTP, streaming, Tauri commands.
- **Swift VMM helper**: `gptbot-vmm` — Virtualization.framework + Virtio socket (legacy binary name).

## Local Computer stack

```text
React → vm_* commands
Rust VirtualMachineManager
  ↓ Unix socket JSON
gptbot-vmm (Swift)
  ↓ Virtualization.framework
Linux guest
  ↓ Virtio / vsock
gptbot-guest-agent (JSON RPC, protocol v1)
```

`LocalMacComputer` maps `AgentComputer` ops to guest methods: `list_dir`, `read_file`, `write_file`, `exec`.

## Persistence

- SQLite: `gptbot.sqlite3` under app data (**legacy filename**).
- `messages` — conversation content + legacy structured tool/status rows (`kind`).
- `agent_runs` — execution observability (`bot_id`, `model`, `computer_id`, `started_at`, `finished_at`, …).
- `run_events` — append-only runtime events (dual-written during UI migration).

## Secret storage

Keychain service `com.drewsepeczi.gptbot` (**legacy compatibility id**). Keys are not stored in SQLite or logged.

## Streaming events

Tauri event: `gptbot://chat-stream` (**legacy compatibility id**). `TauriEventSink` maps `AgentEvent` variants to the existing `StreamEventPayload` shape.

## Cloud

See `docs/CLOUD_ARCHITECTURE.md`. Phase 3A ships `SpriteComputer`; Phase 3B adds the cloud host (Postgres run store, event sink, authenticated run API).
