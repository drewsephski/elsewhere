# Elsewhere

**Create AI workers. Give them computers.** Elsewhere combines persistent bots, ChatGPT/Codex subscription execution, durable work history, private computers, and human approvals.

<img width="1774" height="887" alt="ChatGPT Image Sep 18, 2026, 01_49_41 PM" src="https://github.com/user-attachments/assets/2c9aca1f-6613-4305-95de-a5dc1dbd6c5a" />

---

The product lives in [`apps/www`](apps/www) (Next.js + Better Auth). The Rust control plane is [`services/cloud-host`](services/cloud-host). The macOS Tauri app in the repo root is still available for local/desktop experiments. For full setup, see [authenticated cloud setup](docs/PHASE_3C1.md) and [product progress](docs/PRODUCT_PROGRESS.md).

## Prerequisites

- [Node.js](https://nodejs.org/) 20+ and [pnpm](https://pnpm.io/) 9+
- [Rust](https://rustup.rs/) (stable)
- Postgres for the web app and control plane (local or hosted)
- macOS only if you run the Tauri shell

## Quick start (web)

```bash
pnpm install
cp .env.example .env.local   # fill DATABASE_URL, auth, and cloud-host vars
pnpm --filter @elsewhere/www auth:migrate
cargo run -p cloud-host
pnpm dev:www
```

Sign in at `/sign-in`, open `/app`, create a computer and bot, then delegate work from the dashboard.

## ChatGPT / Codex auth

You do **not** paste an OpenAI API key to use the default product path. Link your **ChatGPT subscription** from the dashboard via Codex device sign-in (`chatgptDeviceCode`): you get a verification code and link, Codex stores credentials on the runner profile volume, and Elsewhere only keeps a profile reference.

- Set `ELSEWHERE_ALLOW_CODEX_LOGIN=1` on cloud-host and point `ELSEWHERE_CODEX_PROFILES_DIR` at a private, persistent directory on the runner (see [Phase 3C.1](docs/PHASE_3C1.md)).
- Automatic provider selection uses subscription execution only; it does not fall back to paid API usage.
- Optional `OPENAI_API_KEY` in `.env` is for explicit Responses/API billing when you choose that provider—not for everyday ChatGPT pairing.

## Development

| What | Command |
|------|---------|
| Web app | `pnpm dev:www` |
| Control plane | `cargo run -p cloud-host` |
| Desktop (Tauri) | `pnpm tauri dev` (starts the web workspace UI on `:1420` and proxies `/api` to `:3000`; run `cargo run -p cloud-host` separately). Better Auth trusts `:1420` by default so sign-in works through the proxy. |
| Frontend only (Vite) | `pnpm dev` |
| Full local check (mirrors CI; needs Postgres for cloud-host integration tests) | `pnpm check` |
| Rust logs | `RUST_LOG=elsewhere=info cargo run -p cloud-host` |

Hosted operations: [`infra/fly/README.md`](infra/fly/README.md).

## Desktop build (optional)

```bash
pnpm tauri build
```

The `.app` bundle is under `src-tauri/target/release/bundle/macos/`. The desktop shell renders the same workspace UI as [`apps/www`](apps/www) (shared components, theme, and routes). Keep the Next.js app running on port 3000 during development so auth and `/api/cloud` BFF routes work through the Vite proxy.

For a packaged build against hosted alpha (no local Next.js), set `ELSEWHERE_DESKTOP_WEBVIEW_URL` to your deployed web app (for example `https://elsewhere-alpha-web.fly.dev/app`) before launching the `.app`. The shell navigates the webview to that URL on startup.

## Testing

```bash
pnpm lint && pnpm test
pnpm --filter @elsewhere/www lint
cargo test -p cloud-host --features test-utils
cd src-tauri && cargo test
```

## Typical flow

1. Sign in to Elsewhere and connect ChatGPT (device sign-in) when prompted.
2. Create a computer, then a bot with a role and instructions.
3. Send work—stream progress, approve tool use when needed, and find results in the dashboard.
4. Refresh or come back later; history, routines, and saved artifacts persist in Postgres.

Layering and roadmap: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md), [`docs/ROADMAP.md`](docs/ROADMAP.md).
