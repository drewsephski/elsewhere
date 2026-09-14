# Hosted alpha operations

Run commands from the repository root. The user approved two Fly Machines, one encrypted 3 GB profile volume, and at most one new acceptance Sprite on September 14, 2026. Supabase Free is already configured; see `docs/SUPABASE_ALPHA.md`. No production DNS, dedicated IPv4, paid database upgrade, or remote build service is authorized by this setup.

## Cost-conscious acceptance

Keep the runner running continuously. It schedules work without inbound HTTP requests, so proxy autostop must remain **off**. The web app can stop between visits and start on the next request; it has no background dispatcher. Validate a cold web start and JWKS retrieval before accepting the deployment.

At current list prices the continuously running runner plus 3 GB volume is approximately **$11.56/month**, plus web runtime (up to $3.32/month), stopped web rootfs, snapshots, network, and Sprite usage. Running both Machines continuously for 48 hours is roughly **$1 base cost**, excluding variable work and any storage retained afterward. This is an estimate, not a prepaid commitment or hard cap. Keep the whole alpha within the approved under-$20 target; do not automatically resize. [Fly billing](https://fly.io/docs/about/billing/).

The acceptance window is a testing plan, not an automatic shutdown timer. After the test, pause its routine and either keep the approved alpha running or explicitly stop the Machines. For a persistent pause, first disable autostart on both Machines (`fly machine update ... --autostart=false`), then stop using SIGTERM with 300 seconds of shutdown grace for the runner. Otherwise an HTTP request can restart a stopped Machine. Retain the profile volume and Supabase data; they must not be deleted just to stop compute charges. Starting again must restore autostart for the web app.

## Build and verify before deployment

```sh
pnpm lint
pnpm --filter @elsewhere/www exec next typegen
pnpm --filter @elsewhere/www lint
pnpm test
fly config validate -c infra/fly/runner.toml
fly config validate -c infra/fly/web.toml

docker build --platform linux/amd64 -f infra/fly/Dockerfile.runner -t elsewhere-runner:alpha .
docker build --platform linux/amd64 -f infra/fly/Dockerfile.web \
  --build-arg NEXT_PUBLIC_BETTER_AUTH_URL=https://elsewhere-alpha-web.fly.dev \
  --build-arg NEXT_PUBLIC_ELSEWHERE_CLOUD_HOST_URL=https://elsewhere-alpha-runner.fly.dev \
  -t elsewhere-web:alpha .
```

Build sequentially on a laptop and check free disk first. Use at least several GB of free space and stop builds before the host disk fills. Docker's source context is allowlisted and excludes `.env*`, personal profiles, Git metadata, dependencies, and local build output. Never send the whole local environment to Fly. Build images contain only placeholder auth configuration; runtime secrets are separate.

The runner image pins Codex 0.154.0 and Rust 1.94. Its build runs the app-server protocol probe, a real MCP configuration parse with an invalid-exposure negative control, and a Linux device-schema/handshake check in a disposable disconnected profile. It must not use the operator's personal Codex directory or API key. Record the final image digests in the acceptance report. The web image runs as `node`; runner startup validates the actual mount, creates its 0700 profile directory, disables core dumps, and drops to UID 10001 before launching the service.

Before publishing, run the web image with 512 MB and an isolated local test database: verify missing/incorrect invitation rejection, a valid disposable signup, a subsequent authenticated request, and no OOM kills. Run the runner image without a mount and require startup failure, then with a disposable mounted directory and local test DB to verify readiness and shutdown. Never run destructive tests against hosted Supabase.

## Provisioning and deployment

The apps `elsewhere-alpha-runner` and `elsewhere-alpha-web` are in the existing `personal` organization. Inventory apps, Machines, and volumes first to avoid duplicate resources. Their shared IPv4 and IPv6 addresses use Fly hostnames only.

Create the runner volume once, after image validation:

```sh
fly volumes create codex_profiles -a elsewhere-alpha-runner --region ord \
  --size 3 --snapshot-retention 7 --scheduled-snapshots --yes
```

Volumes are encrypted by default; never add `--no-encryption`. Stage each service's secret values through `fly secrets import --stage` using stdin. Runtime DB URLs require `sslmode=verify-full` and `sslrootcert=/app/infra/certs/supabase-prod-ca-2021.crt`.

| App | Secrets |
| --- | --- |
| Runner | `DATABASE_URL`, `SPRITE_TOKEN`, `ELSEWHERE_CLOUD_API_TOKEN` |
| Web | `BETTER_AUTH_DATABASE_URL`, `BETTER_AUTH_SECRET`, `ELSEWHERE_ALPHA_INVITE_CODE` |

The web invite code is only for initial account creation; existing sessions/sign-in do not need it. Missing hosted invite configuration fails closed. The public build/runtime flag only controls whether the signup form displays that field; it never contains the secret. The database user-creation hook covers every account-creation path, and the email endpoint rejects bad invitations before password hashing. Disable unused social providers. Keep `.env.hosted` and the separate `.env.alpha-invitation` operator handoff file ignored and mode 0600.

Push the locally tested images with `fly auth docker`, tag them under `registry.fly.io/<app>:<release>`, and `docker push`. Deploy the resulting digest with:

```sh
fly deploy -c infra/fly/web.toml --image registry.fly.io/elsewhere-alpha-web@sha256:WEB_DIGEST --ha=false --strategy immediate
fly deploy -c infra/fly/runner.toml --image registry.fly.io/elsewhere-alpha-runner@sha256:RUNNER_DIGEST --ha=false --strategy immediate
```

These commands use prebuilt images and must not provision a remote builder. Use **exactly one Machine per app**; never deploy a canary/second runner. Runner updates cause a brief maintenance interruption. Verify actual Machine restart policy `always`, 300-second kill timeout, volume attachment, and instance count after deployment. Inspect `/ready`, JWT rejection for anonymous API requests, auth/JWKS availability, and invitation denial before handing the URL to a user.

## Recovery and evidence

The runner holds one database session advisory lock before migrations/recovery. Graceful shutdown stops new claims and allows active work up to 240 seconds, then aborts/joins local execution before releasing leadership. A restart marks potentially executed work interrupted; it must not replay external commands. Untouched queued work survives. A five-second failure of the leadership heartbeat stops the host. `/ready` reports stale dispatch and database availability; further stalled-process watchdog behavior remains a separate operating proof.

Before upgrades, record image, volume, snapshot, and migration IDs. Quiesce pairing/work and take an opaque encrypted volume snapshot; do not inspect individual credential files. Restore into a separate volume only with the required resource/recovery approval, fence the old runner, then verify through Codex account status. Restored refresh credentials may require normal re-pairing. Supabase Free still requires the separately documented logical-backup job and restore drill before inviting additional users.

The full acceptance remains `docs/HOSTED_ALPHA_PROPOSAL.md`: real account and device authorization, restart with pairing retained, observed work and routine completion while the laptop is offline, result download/hash, and the same Sprite sentinel/resource after restart. Image builds, health checks, local fixture sessions, and an empty profile are not that proof.
