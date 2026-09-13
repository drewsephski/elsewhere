# GPT Bot

Native macOS desktop app for persistent AI Bots with OpenAI-backed chat, local SQLite storage, and streaming responses.

## Prerequisites

- macOS (primary target for Phase 1)
- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 20+
- [pnpm](https://pnpm.io/) 9+

## Installation

```bash
pnpm install
```

Rust dependencies are fetched automatically on first `cargo` / `tauri` build.

## API configuration

1. Launch the app.
2. Open **Settings** from the sidebar.
3. Paste your [OpenAI API key](https://platform.openai.com/api-keys).
4. The key is stored in the **macOS Keychain** (service `com.drewsepeczi.gptbot`). It is not written to SQLite or application logs.

Model names in the selector come from the OpenAI **Models** API for your account.

## Development

Start the Tauri dev shell (Vite + Rust):

```bash
pnpm tauri dev
```

Frontend only (no native commands):

```bash
pnpm dev
```

Structured Rust logs (no secrets):

```bash
RUST_LOG=gptbot=info pnpm tauri dev
```

## Running the application

Production build:

```bash
pnpm tauri build
```

The `.app` bundle is emitted under `src-tauri/target/release/bundle/macos/`.

## Testing

```bash
# TypeScript
pnpm lint
pnpm test

# Rust (SQLite persistence + stream helpers)
cd src-tauri && cargo test
```

## Phase 1 workflow

1. Configure OpenAI API key.
2. Create a Bot, pick a model, set system instructions.
3. Send a message and watch the streamed reply.
4. Quit and relaunch — Bots, model selection, and conversation history remain.

See `docs/ARCHITECTURE.md` for layering and `docs/ROADMAP.md` for later phases.
