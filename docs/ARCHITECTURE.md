# GPT Bot — Architecture

## Process boundary

- **Tauri webview**: React UI (TypeScript).
- **Rust host**: SQLite, Keychain, OpenAI HTTP, streaming, Tauri commands and events.
- **Swift VMM helper** (macOS only): `gptbot-vmm` — Virtualization.framework Linux VM + Virtio socket bridge.

The UI never talks to OpenAI directly. All provider traffic runs in Rust.

## Frontend responsibilities

- Layout: sidebar, bot settings strip, conversation, composer, modals.
- **Application services** (`src/services/*`): orchestrate bots, chat, and (Phase 2A) VM diagnostics.
- **ModelProvider** (`src/providers/*`): thin abstraction over Tauri; OpenAI is the only implementation today.
- Subscribe to `gptbot://chat-stream` for incremental assistant tokens.
- Zod-validated types in `src/lib/definitions.ts` and `src/services/vm-service.ts`.
- **VM diagnostics panel** (`src/ui/vm-diagnostics-panel.tsx`): developer spike UI — not the final Agent Computer viewer.

## Rust responsibilities

- **Database** (`src-tauri/src/db`): bots, conversations, messages (with `kind` for future non-text events).
- **Secrets** (`src-tauri/src/secrets`): Keychain-backed OpenAI API key.
- **OpenAI** (`src-tauri/src/openai`): list models, SSE chat completions stream.
- **Commands** (`src-tauri/src/commands`): CRUD + `start_chat` / `cancel_chat` + `vm_*` (macOS).
- **Virtual machine** (`src-tauri/src/vm`): `VirtualMachineManager`, provisioning, guest RPC.
- **Logging**: `tracing` with structured fields — never API keys.

SQLite path: app data directory / `gptbot.sqlite3`.

## Phase 2A — Local Computer (spike)

```text
React
  ↓ invoke(vm_*)
Rust VirtualMachineManager
  ↓ Unix socket (JSON)
gptbot-vmm (Swift)
  ↓ Virtualization.framework
Linux guest (Alpine on root.raw)
  ↓ Virtio socket / AF_VSOCK
gptbot-guest-agent
```

### VM process boundary

- Rust **spawns** `gptbot-vmm serve --config … --socket …` and speaks JSON over `vm/runtime/vmm.sock`.
- Swift process owns the `VZVirtualMachine` instance and `VZVirtioSocketDevice.connect(toPort:)` for guest RPC.

### VM storage layout

See `docs/VIRTUALIZATION_SPIKE.md`. All persistent state lives under `app_data/vm/`.

### Guest boot strategy

Alpine `vmlinuz-virt` + `initramfs-virt`, kernel cmdline `root=/dev/vda`, writable virtio block disk.

### Host ↔ guest protocol

Newline-delimited JSON; methods `ping`, `exec`, `read_file`, `write_file`; `protocolVersion: 1`.

### Lifecycle

`VirtualMachineManager` tracks `notCreated | stopped | starting | running | stopping | error` and surfaces VMM/guest errors to the UI.

### Security assumptions (spike)

- Single-user local app; guest agent runs as root in VM (Alpine default) — **not** production hardening.
- No secrets in guest image; no hardcoded passwords.
- VM entitlement required on host; failures are explicit.

## Provider abstraction

| Layer | Role |
|--------|------|
| `ModelProvider` (TS) | UI-facing; swap providers without rewriting chat UI |
| Tauri commands | Stable IPC surface |
| `OpenAiClient` + `stream_chat_completion` (Rust) | First provider |

## Secret storage

- Interface: `SecretStore` in Rust.
- Production: `KeychainSecretStore` via the `keyring` crate.
- Keys are not logged, not stored in the DB, not passed to the webview except on save.

## Conversation flow

1. User sends message → `start_chat`.
2. Rust persists user message + empty assistant row (`streaming`).
3. Background task streams OpenAI SSE; appends body in SQLite; emits `delta` events.
4. On success → `complete` + `done` event; on failure → `error` row + `error` event.
5. `cancel_chat` sets an atomic flag checked during SSE read.

## Extensibility hooks

- `messages.kind` for tool/runtime events later.
- `provider` column on bots for additional model backends.
- Rust `SecretStore` + TS `ModelProvider` for new auth mechanisms.
- Guest RPC protocol version field for Phase 2B tool surface.
