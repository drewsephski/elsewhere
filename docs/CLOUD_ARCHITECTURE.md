# Elsewhere — Cloud boundary

Phase **3A** adds a Fly Sprites-backed `SpriteComputer` crate. The portable runtime in `agent-core` still has **zero Fly imports**.

## AgentComputer

All workspace tools (`workspace_list`, `workspace_read`, `workspace_write`, `workspace_exec`) go through the async `AgentComputer` trait in `crates/agent-core`:

```text
AgentComputer (async)
 ├── LocalMacComputer   — desktop: Virtualization.framework + guest-agent RPC
 └── SpriteComputer     — cloud: Fly Sprites REST API (no guest-agent)
```

The Luna / OpenAI Responses tool loop never imports Tauri, SQLite, VM code, or Fly SDK types.

### Why SpriteComputer does not use `gptbot-guest-agent`

Fly Sprites already expose host-level filesystem, exec, checkpoints, and network policy over HTTPS. Mapping those endpoints directly to `AgentComputer` avoids installing a redundant daemon inside every Sprite.

`gptbot-guest-agent` remains the transport for:

- `LocalMacComputer` (Virtio socket JSON RPC)
- Future microVM backends (e.g. Firecracker)

The portable contract is **`AgentComputer`**, not a single RPC protocol.

## Host adapters (today)

| Trait | Desktop | Cloud (3A) |
|--------|---------|------------|
| `AgentComputer` | `LocalMacComputer` | `SpriteComputer` (`crates/sprite-computer`) |
| `RunStore` | `SqliteRunStore` | *(Phase 3B — Postgres)* |
| `EventSink` | `TauriEventSink` | *(Phase 3B — SSE/WebSocket)* |
| `ResponsesModel` | `OpenAiResponsesModel` | `OpenAiResponsesModel` (API key fallback) |
| `RunEngine` | `ResponsesRunEngine` | `ResponsesRunEngine` today; `CodexRunEngine` in 3B.2 |

```text
Elsewhere host process
    ↓ SPRITES_TOKEN (never stored in Sprite /workspace)
SpriteComputer
    ↓ https://api.sprites.dev/v1/...
Fly Sprite (persistent /workspace, sleeps when idle)
```

There is **no** cloud control plane, public Sprite URL exposure, or Luna-in-Sprite deployment in Phase 3A.

## Identity

Elsewhere separates:

| Field | Meaning |
|-------|---------|
| `bot_id` | Product bot record |
| `computer_id` | Logical sandbox/computer in Elsewhere |
| `sprite_name` | Provider resource id, e.g. `elsewhere-<sandbox-id>` |

Use `sprite_name_for_sandbox()` in `sprite-computer` — do not derive Fly names from user-facing bot titles.

## Security defaults

New Sprites created by `SpriteComputer` receive a **default-deny** egress policy (`deny *`). Workspace paths are constrained to `/workspace` even though the Sprites API accepts broader paths.

## Legacy compatibility identifiers

| Identifier | Purpose |
|------------|---------|
| `com.drewsepeczi.gptbot` | Tauri app id, Keychain service |
| `gptbot.sqlite3` | Local database filename |
| `gptbot://chat-stream` | Tauri stream event name |
