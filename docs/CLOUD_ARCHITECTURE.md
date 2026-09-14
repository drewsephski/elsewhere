# Elsewhere — Cloud boundary (planned)

This document describes the **portable runtime boundary** only. There is no cloud computer implementation in the repository yet.

## AgentComputer

All workspace tools (`workspace_list`, `workspace_read`, `workspace_write`, `workspace_exec`) go through `AgentComputer` in `crates/agent-core`:

```text
AgentComputer
 ├── LocalMacComputer   (desktop — Virtualization.framework + guest-agent)
 └── CloudComputer    (future — e.g. Fly Sprites adapter)
```

The Luna / OpenAI Responses tool loop in `agent-core` never imports Tauri, SQLite, or VM code.

## Desktop host adapters

| Trait | Desktop implementation |
|--------|-------------------------|
| `AgentComputer` | `LocalMacComputer` → `VirtualMachineManager` → guest JSON RPC |
| `RunStore` | `SqliteRunStore` |
| `EventSink` | `TauriEventSink` → `gptbot://chat-stream` |
| `ResponsesModel` | `OpenAiResponsesModel` |

## Planned first cloud host

**Fly Sprites** is the intended first `CloudComputer` backend (not a dependency today). A future `SpriteComputer` should implement `AgentComputer` only — no changes to `run_agent_loop`.

## Legacy compatibility identifiers

These remain on desktop installs until a dedicated migration:

| Identifier | Purpose |
|------------|---------|
| `com.drewsepeczi.gptbot` | Tauri app id, Keychain service |
| `gptbot.sqlite3` | Local database filename |
| `gptbot://chat-stream` | Tauri stream event name |
