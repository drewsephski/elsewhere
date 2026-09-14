---
name: fly-deploy
description: Deploys Elsewhere alpha to Fly.io (runner and web), applies SQLx migrations via runner startup or local sqlx CLI, and runs post-deploy smoke checks. Use when the user asks to deploy to Fly, redeploy hosted alpha, ship to elsewhere-alpha-runner/web, or run hosted database migrations before deploy.
---

# Fly deploy (Elsewhere alpha)

Run all commands from the **repository root**.

## Apps and configs

| Service | Fly app | Config |
| --- | --- | --- |
| cloud-host (runner) | `elsewhere-alpha-runner` | `infra/fly/runner.toml` |
| Next.js (web) | `elsewhere-alpha-web` | `infra/fly/web.toml` |

Public URL: https://elsewhere-alpha-web.fly.dev/

Constraints: **one Machine per app**, runner autostop **off**, `--ha=false`, `--strategy immediate`. Do not add a second runner Machine.

## Quick deploy

```sh
./.cursor/skills/fly-deploy/scripts/deploy.sh
```

Or manually (runner first so migrations and API are current before web):

```sh
fly deploy -c infra/fly/runner.toml --ha=false --strategy immediate
fly deploy -c infra/fly/web.toml --ha=false --strategy immediate
```

`fly deploy` builds and pushes via Fly/Depot; it does **not** require a separate remote builder when using these configs.

## Migrations

### SQLx (control plane) — default path

New files under `services/cloud-host/migrations/` ship in the runner image. On startup the leader runs `sqlx::migrate!("./migrations")` before serving traffic.

**If you added or changed SQLx migrations:** redeploy the **runner** (full deploy script is fine). Check runner logs for migration errors; `/ready` should return 200.

### SQLx — manual (optional)

Use when you need to apply migrations without a full deploy, or to inspect pending versions:

```sh
./.cursor/skills/fly-deploy/scripts/migrate-hosted.sh info
./.cursor/skills/fly-deploy/scripts/migrate-hosted.sh run
```

Requires `sqlx` CLI (`cargo install sqlx-cli --no-default-features --features rustls,postgres`) and `.env.hosted` with `DATABASE_URL` for `elsewhere_runner`. The script rewrites `sslrootcert` from the Fly image path to `infra/certs/supabase-prod-ca-2021.crt` in the repo.

### Better Auth (web schema)

Only when Better Auth / `apps/www` auth tables change:

```sh
# Load BETTER_AUTH_* from .env.hosted (same sslrootcert rewrite as runner URL if needed)
pnpm --filter @elsewhere/www auth:migrate --yes
```

Supabase bootstrap order and RLS: see [docs/SUPABASE_ALPHA.md](docs/SUPABASE_ALPHA.md).

## Post-deploy smoke

```sh
curl -sf https://elsewhere-alpha-runner.fly.dev/ready
curl -sf https://elsewhere-alpha-web.fly.dev/api/auth/ok
curl -sf https://elsewhere-alpha-web.fly.dev/api/health/runner
```

- `/api/auth/ok` — web process alive (not runner reachability).
- `/api/health/runner` — web BFF can reach runner `/ready`.

For signed-in workspace proof, one authenticated `GET /api/cloud/v1/workspace` as documented in `infra/fly/README.md`.

## Pre-deploy checks (when changing infra or Docker)

```sh
fly config validate -c infra/fly/runner.toml
fly config validate -c infra/fly/web.toml
```

Heavier local validation (lint, tests, docker builds): see [infra/fly/README.md](infra/fly/README.md).

## Secrets

Never commit `.env.hosted`. Runtime secrets live in Fly (`fly secrets list -a elsewhere-alpha-runner` / `elsewhere-alpha-web`). Runner needs `DATABASE_URL` with `sslmode=verify-full` and `sslrootcert=/app/infra/certs/supabase-prod-ca-2021.crt` inside the image.

## Examples

**User:** “Deploy to Fly”  
→ Run `scripts/deploy.sh`, report smoke check results and any Fly DNS warnings (deploy can still succeed).

**User:** “I added migration 016”  
→ Deploy runner (or `migrate-hosted.sh run` then deploy), confirm logs and `/ready`.

**User:** “Only redeploy web”  
→ `fly deploy -c infra/fly/web.toml --ha=false --strategy immediate` and smoke web + `/api/health/runner`.
