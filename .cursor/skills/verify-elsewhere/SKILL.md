---
name: verify-elsewhere
description: Drive the Elsewhere web workspace the way a user does and capture proof. Use when proving UI changes, reproducing a workspace bug, or checking sign-in, landing, computers, approvals, or /app behavior on the Next.js app.
---

# Verify Elsewhere

Elsewhere's primary user surface is the Next.js web app at `http://127.0.0.1:3000` (`apps/www`). Drive it in a real browser. Do not treat Vitest, `cargo test`, or curl of `/v1/*` as UI proof.

Secondary surfaces (do not use these as the default proof path):

- Rust control plane `cloud-host` on `127.0.0.1:8080`
- Optional macOS Tauri shell on `:1420` (proxies `/api` to `:3000`)

Read [features/README.md](features/README.md) before a run. Drive the mapped feature file, not a convenient nearby page.

## Launch

Run from the repository root. Prefer attaching to an instance this run started. If `:3000` already answers, run Doctor and only continue if it is this checkout's web app.

```bash
pnpm install
pnpm --filter @elsewhere/www auth:migrate
pnpm dev:www
```

Ready when `GET http://127.0.0.1:3000/sign-in` returns HTTP 200 and the HTML contains the heading `Sign in` and the product name `Elsewhere`. Next prints a local URL on port 3000.

Workspace APIs also need the control plane:

```bash
cargo run -p cloud-host
```

Ready when `GET http://127.0.0.1:8080/health` returns JSON `"service":"elsewhere-cloud-host"`. `GET /ready` is 200 only when Postgres and the dispatcher heartbeat are healthy.

Env lives in `apps/www/.env` and/or repo `.env` (both gitignored). Required for a full stack: `DATABASE_URL`, Better Auth vars, `ELSEWHERE_CLOUD_HOST_URL=http://127.0.0.1:8080`, `SPRITE_TOKEN`, `ELSEWHERE_CLOUD_API_TOKEN`. Local Postgres from `infra/dev/docker-compose.yml` is `postgres://elsewhere:elsewhere@127.0.0.1:5432/elsewhere`.

Do not start a second `pnpm dev:www` on 3000. Do not start a second `cloud-host` against the same `DATABASE_URL` (advisory lock, one dispatcher).

Record PIDs of processes this run started under `/tmp/elsewhere-verify-$RUN_ID/`.

## Doctor

```bash
.cursor/skills/verify-elsewhere/scripts/doctor.sh
```

Pass means the web app on `ELSEWHERE_VERIFY_BASE` (default `http://127.0.0.1:3000`) serves `/sign-in` as Elsewhere. The script also reports cloud-host `/health` and www `/api/health/runner` without failing the landing or sign-in recipes when those are down.

If Doctor fails, fix launch. Do not drive a foreign process on 3000.

## Drive

Use the Cursor browser tools (`browser_navigate`, `browser_snapshot`, `browser_click`, `browser_fill`, `browser_take_screenshot`). Prefer accessible names and `apps/www/lib/app-routes.ts` paths.

Stable handles:

- Landing home link: accessible name `Elsewhere home`
- Landing primary nav: `Product`, `Bots`, `Computers`, `Source`
- Landing header CTA: link `Open workspace` to `/app`
- Landing hero: button `Get started` (opens the get-started dialog)
- Get-started dialog: heading `How do you want to start?`, link `Open workspace now` to `/app`
- Sign-in page (`/sign-in`): heading `Sign in`, textboxes `Email` and `Password`, button `Sign in`, link `Need an account? Sign up`, link `Back to home`
- Unauthenticated `/app`: redirects to `/sign-in`
- Signed-in workspace: `/app`. Legacy management nav (`aria-label="App navigation"`) uses labels from `appShellNavLinks`: `Work`, `Approvals`, `Results`, `Routines`, `Computers`, `Integrations`, `Channels`, `Skills`, `Settings`

Do not sign in with production credentials in a shared browser profile. Use a disposable account or stop at the sign-in form when the recipe is unauthenticated.

## Evidence

Write proof under `.cursor/skills/verify-elsewhere/evidence/<run-id>/`. Keep that directory. Cleanup must not delete it.

For a UI proof, capture all of:

1. The action (ARIA snapshot or labeled click)
2. The resulting screen (screenshot with `Elsewhere` visible)
3. A second view of the same fact (URL, heading, or a follow-up GET)

Name files with the feature id, for example `sign-in/form.aria.yml` and `sign-in/form.png`.

Mocks are only valid at production boundaries (Fly sprites, Slack, ChatGPT device login). Do not stub Better Auth or the BFF for workspace proofs.

## Cleanup

Kill only PIDs this run recorded. Never `pkill -f next` or `pkill -f cloud-host`. Leave evidence on disk.

```bash
if [ -f /tmp/elsewhere-verify-$RUN_ID/www.pid ]; then
  kill "$(cat /tmp/elsewhere-verify-$RUN_ID/www.pid)" || true
fi
if [ -f /tmp/elsewhere-verify-$RUN_ID/cloud-host.pid ]; then
  kill "$(cat /tmp/elsewhere-verify-$RUN_ID/cloud-host.pid)" || true
fi
```

If you attached to an already-running app, do not stop it.

## Helpers

- [scripts/doctor.sh](scripts/doctor.sh) is the read-only instance check. Run it before every drive.
