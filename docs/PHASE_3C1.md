# Phase 3C.1 — Authenticated control plane

Elsewhere user accounts (Better Auth) are separate from ChatGPT/Codex runner auth.

## Architecture

| Layer | Responsibility |
|--------|----------------|
| `@elsewhere/www` | Better Auth (`auth` PG schema), sessions, ES256 JWT plugin, dashboard |
| `cloud-host` | JWT + optional internal token (`hybrid`), resource ownership, APIs |
| Codex app-server | Subscription execution only; credentials stay on runner |

### JWT alignment

- **Issuer:** `BETTER_AUTH_URL` → `ELSEWHERE_JWT_ISSUER`
- **Audience:** `elsewhere-cloud-host` → `ELSEWHERE_JWT_AUDIENCE`
- **JWKS:** `{BETTER_AUTH_URL}/api/auth/jwks` → `ELSEWHERE_JWT_JWKS_URL`

### Auth modes (`ELSEWHERE_AUTH_MODE`)

- `hybrid` — Bearer internal token **or** Better Auth JWT (local dev + E2E)
- `jwt` — product deployments
- `internal_token` — legacy scripts only

Internal token maps to principal `legacy-local`.

## Local setup

1. Postgres with `DATABASE_URL` (SQLx migrations on `public` schema).
2. Copy `.env.example` → `.env.local` (www + cloud-host vars).
3. `pnpm --filter @elsewhere/www auth:migrate` — Better Auth tables in `auth` schema.
4. `cargo run -p cloud-host` and `pnpm --filter @elsewhere/www dev`.
5. Sign in at `/sign-in`, use `/app` dashboard.

### Codex login on runner

Set `ELSEWHERE_ALLOW_CODEX_LOGIN=1` on cloud-host. UI exposes runner-local OAuth start (no token storage in Elsewhere DB).

## Product APIs (JWT)

- Bots / computers CRUD
- `POST /v1/runs` with `{ botId, conversationId?, message }` (no client-supplied instructions/computer/provider IDs)
- `GET /v1/providers/status`
- Legacy bootstrap run shape: **internal token only**

## Manual acceptance checklist

- [ ] Sign in to Elsewhere
- [ ] Dashboard shows provider status and your runs only
- [ ] Create computer → create Luna bot → chat with tool activity + final answer
- [ ] Refresh; history persists; second run sees persistent workspace file

## Tests

```bash
cargo test -p cloud-host --features test-utils
cargo test -p agent-core -p computer-mcp
pnpm --filter @elsewhere/www lint
```
