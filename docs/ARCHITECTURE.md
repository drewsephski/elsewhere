# GPT Bot — Phase 1 Architecture

## Process boundary

- **Tauri webview**: React UI (TypeScript).
- **Rust host**: SQLite, Keychain, OpenAI HTTP, streaming, Tauri commands and events.

The UI never talks to OpenAI directly. All provider traffic runs in Rust.

## Frontend responsibilities

- Layout: sidebar, bot settings strip, conversation, composer, modals.
- **Application services** (`src/services/*`): orchestrate bots and chat.
- **ModelProvider** (`src/providers/*`): thin abstraction over Tauri; OpenAI is the only implementation today.
- Subscribe to `gptbot://chat-stream` for incremental assistant tokens.
- Zod-validated types in `src/lib/definitions.ts`.

## Rust responsibilities

- **Database** (`src-tauri/src/db`): bots, conversations, messages (with `kind` for future non-text events).
- **Secrets** (`src-tauri/src/secrets`): Keychain-backed OpenAI API key.
- **OpenAI** (`src-tauri/src/openai`): list models, SSE chat completions stream.
- **Commands** (`src-tauri/src/commands`): CRUD + `start_chat` / `cancel_chat`.
- **Logging**: `tracing` with `request_id`, `bot_id`, `conversation_id`, `message_id` — never API keys.

SQLite path: app data directory / `gptbot.sqlite3`.

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

## Extensibility hooks (not built in Phase 1)

- `messages.kind` for tool/runtime events later.
- `provider` column on bots for additional model backends.
- Rust `SecretStore` + TS `ModelProvider` for new auth mechanisms.
