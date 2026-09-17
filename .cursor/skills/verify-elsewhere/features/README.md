# Elsewhere verification map

This directory is the maintained source for verifying user-facing behavior of the Elsewhere web app. Read this index before driving the app, then use the matching feature file as the recipe.

## Baseline preconditions

- Web app at `http://127.0.0.1:3000` from this checkout (`pnpm dev:www`).
- Run `.cursor/skills/verify-elsewhere/scripts/doctor.sh` and require `web: ok`.
- Never drive an instance Doctor rejected.
- Unauthenticated recipes stop at public pages. Do not reuse the operator's signed-in browser profile.
- Workspace recipes need a disposable signed-in session plus `cloud-host` with `GET /health` 200.
- One `cloud-host` per `DATABASE_URL`. Do not start a second dispatcher against the same database.

## Driving conventions

- Start every recipe from the baseline unless its preconditions say otherwise.
- Prefer ARIA roles, accessible names, labeled inputs, and paths in `apps/www/lib/app-routes.ts`.
- Treat quoted names and routes as literal.
- Restore any account or computer this run created. Keep proof artifacts.

## Proof and skip reporting

- Capture the user action and the resulting state, not only the final screen.
- UI proof includes an ARIA snapshot and a screenshot with the product name visible.
- Mutation proof includes a second read of the stored value (reload, second page, or API GET through the same-origin BFF).
- Record the feature ID and entry point with every artifact under `.cursor/skills/verify-elsewhere/evidence/<run-id>/`.
- Report an unreachable path with the attempted step and the unmet precondition.
- Do not report a skipped entry point as verified through a different path.

## Features

- [Landing](./landing.md) covers the public marketing page, Get started, and Open workspace.
- [Sign in](./sign-in.md) covers `/sign-in`, the email form, sign-up toggle, and the unauthenticated `/app` redirect.
- [Workspace](./workspace.md) covers the signed-in `/app` chat shell.
- [Computers](./computers.md) covers `/app/computers`.
- [Approvals](./approvals.md) covers `/app/approvals`.
