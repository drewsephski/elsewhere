# Elsewhere hosted alpha — approval proposal

Status: **Supabase Free approved and provisioned; Fly services not provisioned or deployed**. Revised September 14, 2026 after the user selected Supabase and “drew's projects.” This replaces the earlier $59.33/month Managed Postgres proposal. Fly authentication is restored; read-only inventory shows no apps in the personal organization. The existing Sprite `elsewhere` is an agent-computer test resource, not a deployment host. See [the live database setup and verification record](SUPABASE_ALPHA.md).

## Recommendation and exact resource request

Use the existing Rust API/dispatcher/worker as **one persistent runner**, a separate Next.js web/auth process in Chicago (`ord`), and **Supabase Free Postgres in us-east-2**. Use the existing Fly personal organization; no new organization or subscription plan is requested. Names below are requested names, subject to availability.

| Resource | Requested name | Quantity and configuration | Purpose |
| --- | --- | --- | --- |
| Fly runner app | `elsewhere-alpha-runner` | Exactly 1 Machine, shared-cpu-1x, 2 GB RAM, `ord`, always running | Rust API, dispatcher, routines, Codex child processes |
| Fly web app | `elsewhere-alpha-web` | Exactly 1 Machine, shared-cpu-1x, 512 MB RAM, `ord`, always running | Next.js, Better Auth, web UI and private download proxy |
| Supabase project (created) | `Elsewhere Alpha` / `edbfhcveqxtxnfybhmej` | Free plan, PostgreSQL 17.6, 500 MB database allowance, `us-east-2`, organization “drew's projects” | Durable control plane, accounts, approvals, results |
| Private profile volume | `codex_profiles` on runner app | Exactly 1 encrypted 3 GB Fly Volume; mount `/var/lib/elsewhere`; daily snapshots retained 7 days | Codex-managed private per-owner profiles |
| Public endpoints | Each app's `*.fly.dev` hostname | Shared IPv4 and IPv6, managed HTTPS | Browser access without changing production DNS |
| Acceptance computer | Product-generated Sprite ID | At most 1 **new** Sprite, created through the signed-in product flow after approval | Prove owner assignment and durable computer state |

No dedicated IPv4, second runner, Redis, object-storage service, Kubernetes, fleet manager, new Sprite plan, or production-domain changes. The existing test Sprite is left untouched. Adding more alpha computers or increasing service sizes requires a separate cost decision.

The web app and runner use separate images and secrets. Rebuilding web assets cannot overwrite the profile volume or restart active Codex work. Both services must be hosted: leaving Better Auth or Postgres on the laptop would still break the product promise.

## Estimated monthly cost

Estimate for 30 days, always-on apps, initially Drew and then 1–5 invited users, one concurrent assignment. These are list-price estimates, not a quote or a hard billing cap; no trial credit, existing plan allowance, or reservation discount is assumed.

| Item | Monthly estimate |
| --- | ---: |
| Runner, 1 shared CPU / 2 GB | $11.11 |
| Web/auth, 1 shared CPU / 512 MB | $3.32 |
| Supabase Free Postgres | $0.00 |
| Profile volume, 3 GB × $0.15 | $0.45 |
| **Fixed baseline** | **$14.88** |

Target **under $20/month for a lightly used alpha**, leaving $5.12 above the fixed baseline for Sprite use, snapshots, and network/build overhead. This is a planning envelope, not a hard cap. Busy Sprite work can exceed it; do not promise an unlimited $20 service. Build images locally initially, check actual billing during acceptance, and pause the test routine afterward. Do not resize or enable paid database upgrades automatically.

Supabase Free includes 500 MB database storage and 5 GB egress, has no automatic backups/PITR, and may pause after a week of inactivity. This is a supervised alpha availability tradeoff. Monitor database growth (especially saved artifact bytes and events), back up before inviting users, and surface database unavailability through runner readiness. Normal dispatcher traffic must not be replaced by artificial keep-alive traffic to evade provider policies. Supabase Pro starts at $25/month and would exceed this whole budget. The 512 MB web Machine must pass a production-image memory check; build Next.js outside the runtime Machine. A required larger size triggers a budget review.

Variable charges: profile snapshots cost $0.08/GB-month beyond the organization's first 10 GB; do not assume that free allocation is unused. North American internet egress is $0.02/GB. Sprite compute is $0.07/CPU-hour and $0.04375/GB-hour of memory, with hot/cold storage billed separately. The user's existing ChatGPT subscription is separate; no OpenAI API usage or automatic fallback is budgeted. Taxes and any existing organization subscription fees are excluded. Billing alerts are advisory, not enforced spend caps.

Sources checked September 14: [Fly resource pricing](https://fly.io/docs/about/pricing/), [Supabase plans](https://supabase.com/pricing), [Sprite usage pricing](https://fly.io/sprites/#pricing), [supported regions](https://fly.io/docs/reference/regions/).

## Storage, ownership, and credentials

Postgres stores Elsewhere account ownership, UUID profile references, bot/computer mapping, queued/running work, events, approvals, routines, context, and bounded artifact snapshots. Better Auth uses `elsewhere_auth`, since Supabase reserves `auth` for its own service. The runner uses `elsewhere_control`. The separate `elsewhere_web` and `elsewhere_runner` roles own only their respective schemas, have bounded connections, and cannot access the other schema. They run their existing migrations; splitting owner/runtime roles further is deferred for this alpha. The Data API is disabled. Both private schemas deny Supabase client/service roles, and application tables have owner-only RLS with no client policies. Elsewhere still enforces per-user ownership in its server APIs; RLS does not replace that boundary.

`ELSEWHERE_CODEX_PROFILES_DIR=/var/lib/elsewhere/codex-profiles` is mounted only on the trusted runner. Each owner's UUID directory has mode 0700, the service runs as a dedicated non-root UID, and startup must verify the expected writable volume mount before readiness. The image, root filesystem, temporary directory, Next.js app, and Sprites must not hold the persistent profiles. Configure a restrictive umask and disable core dumps.

Codex alone performs device authorization and writes/refreshes its credential files. Elsewhere must not open, parse, log, export, or copy individual OAuth tokens. A fresh owner must never inherit the operator's account. Encryption at rest is provided by the encrypted Fly Volume; this does not protect credentials against a compromised runner process or privileged administrator. Keep Fly organization access restricted. [Fly Volume creation and encryption](https://fly.io/docs/volumes/volume-manage/).

The control plane uses the **direct IPv6 Supabase** endpoint for `DATABASE_URL`, with `sslmode=verify-full` and the official Supabase CA in `infra/certs/`. SSL enforcement is enabled on the project. SQLx migrations and the dedicated session advisory lock must not use transaction pooling. The alpha web/auth pool also uses a direct endpoint to preserve its configured `search_path`; the web pool is capped at five, with role limits of six web connections and twelve runner connections. [Supabase connection guidance](https://supabase.com/docs/guides/database/connecting-to-postgres).

## Secrets and environment layout

Secrets are entered through Fly's secret storage using stdin or its dashboard, never committed or passed as Docker build arguments. Do not copy the local `.env` wholesale. Exclude all `.env*`, personal Codex directories, build output, and Git internals from Docker contexts. Only the explicitly public values below belong in the Next.js build.

| Service | Secret | Purpose |
| --- | --- | --- |
| Runner | `DATABASE_URL` | TLS direct Supabase URL, control-plane database role |
| Runner | `SPRITE_TOKEN` | Approved Sprite organization access, runner only |
| Runner | `ELSEWHERE_CLOUD_API_TOKEN` | Random server secret required by current configuration; internal-token auth is disabled in JWT-only mode |
| Web | `BETTER_AUTH_DATABASE_URL` | TLS direct Supabase URL, auth database role |
| Web | `BETTER_AUTH_SECRET` | New stable cryptographic session secret |
| Operations only | Fly deploy credentials and migration-role URLs | Kept outside both application runtimes where practical |

Runner non-secret configuration:

```dotenv
ELSEWHERE_AUTH_MODE=jwt
ELSEWHERE_RUN_ENGINE=codex
ELSEWHERE_ALLOW_CODEX_LOGIN=true
ELSEWHERE_CODEX_PROFILES_DIR=/var/lib/elsewhere/codex-profiles
CODEX_EXECUTABLE=/usr/local/bin/codex
ELSEWHERE_MAX_CONCURRENT_RUNS=1
ELSEWHERE_RUN_TIMEOUT_SECS=900
ELSEWHERE_TOOL_APPROVAL_TIMEOUT_SECS=300
ELSEWHERE_BIND=0.0.0.0:8080
ELSEWHERE_JWT_ISSUER=https://elsewhere-alpha-web.fly.dev
ELSEWHERE_JWT_AUDIENCE=elsewhere-cloud-host
ELSEWHERE_JWT_JWKS_URL=https://elsewhere-alpha-web.fly.dev/api/auth/jwks
ELSEWHERE_WEB_ORIGIN=https://elsewhere-alpha-web.fly.dev
RUST_LOG=cloud_host=info,codex_provider=info,sprite_computer=info
```

`OPENAI_API_KEY` is absent. `SPRITES_API_BASE` retains the existing official default. Codex is version-pinned in the image only after the **Linux** build passes the required device-login/MCP schema probes; the locally installed CLI version is not evidence that a distributable Linux build is compatible.

Web runtime: `BETTER_AUTH_URL=https://elsewhere-alpha-web.fly.dev`, `NODE_ENV=production`, `PORT=3000`, `BETTER_AUTH_DATABASE_SCHEMA=elsewhere_auth`. Public build/runtime values:

```dotenv
NEXT_PUBLIC_BETTER_AUTH_URL=https://elsewhere-alpha-web.fly.dev
NEXT_PUBLIC_ELSEWHERE_CLOUD_HOST_URL=https://elsewhere-alpha-runner.fly.dev
NEXT_PUBLIC_ELSEWHERE_JWT_AUDIENCE=elsewhere-cloud-host
```

Use email/password sign-in initially; Google OAuth is unnecessary for this acceptance gate. Restrict initial access to Drew's approved alpha account before enabling public execution; the existing self-signup flow is not an invite gate. Do not expose an unrestricted resource-creation service under an assumption that an obscure URL is private.

## Deployment and operating model

After approval, finish and locally test two immutable Linux images, a secrets-excluding build context, Fly configuration, private-volume startup checks, and initial account admission restriction. Run the existing migrations and pinned Better Auth migration tooling deliberately. The proposal is not a claim that those deployment artifacts already exist or that a Linux Codex image has passed validation.

Provision the listed resources only after authenticated read-only inventory and price/region checks. Keep `auto_stop_machines="off"`, exactly one runner Machine, restart policy `always`, `kill_signal="SIGTERM"`, and `kill_timeout=300`. Disable Fly's automatic extra Machine creation on initial deployment. Use an in-place/immediate single-runner update with a maintenance window, not a canary that competes for leadership or silently creates a second empty profile volume. Brief API unavailability during runner replacement is acceptable for this alpha.

Acquire the dedicated database leadership lock before migrations/recovery. A second runner fails before it can mark another runner's work interrupted. The dispatcher claims queued work transactionally and reserves a bot/computer through artifact collection. On SIGTERM/SIGINT, stop new dispatch and routine admission, report unready, and allow up to 240 seconds for active work; continue checking leadership and processing cancellation. Abort/join remaining local executions before releasing leadership. A remotely accepted Sprite command may still finish; never automatically replay it.

On process or Machine restart, mount the same volume, reacquire leadership, mark potentially executed work interrupted, cancel stale approvals, and dispatch untouched queued work. Routines use the existing occurrence keys and missed-slot coalescing. This is a supervised single-runner design, not distributed fencing or an exactly-once external-effects guarantee. During database failover or ambiguous machine failure, ensure the old runner is stopped before starting a replacement.

Use `/health` for liveness and `/ready` for DB/dispatcher readiness; a health-check failure alone is not a restart mechanism. Runtime fatal errors exit for supervision. Before inviting users, verify the stalled-dispatch watchdog/restart behavior and add a bounded watchdog if the current supervisor cannot detect a live but permanently stuck process. The workspace already shows unavailable-runner state. Keep approvals and their timeout/failure state visible.

Monitor recent dispatcher activity, process restarts, active/queued work, approval waits, interrupted assignments, result collection failures, profile disk capacity, and database storage. Emit service/instance/version identifiers in deployment logs; keep raw Codex debug output, task contents, credentials, and device codes out of retained operational logs. Use Fly's included logs/metrics initially; no additional observability vendor is requested.

Sources: [Fly application configuration](https://fly.io/docs/reference/configuration/), [deployment strategies](https://fly.io/docs/launch/deploy/). Fly describes shutdown grace as best effort, so forced-restart recovery remains an acceptance requirement.

## Backup, rollback, and recovery

- **Profiles:** enable daily encrypted whole-volume snapshots with seven-day retention. Before an upgrade/move, quiesce the runner and pairing processes, stop the Machine, take a whole-volume snapshot, and record volume/snapshot/image IDs with a database recovery timestamp. Snapshot the opaque volume; do not inspect or export credential files. This establishes a consistent operational restore point, while scheduled live snapshots may be only crash-consistent. Fly warns snapshots are not a complete independent backup strategy; before widening beyond the alpha, decide on an additional encrypted backup domain. [Volume snapshots](https://fly.io/docs/volumes/snapshots/).
- **Database:** Supabase Free does not include automatic backups. Before inviting users, implement daily PostgreSQL 17 logical dumps of both application schemas with their owner roles; include the bootstrap role/schema migrations and deployment configuration, but keep passwords separately. Store dumps encrypted on the Fly volume and capture them in volume snapshots, with a separately retained encrypted operator copy before upgrades. Pause web writes and drain the runner for a coordinated two-schema recovery point. A normal daily dump pair is not one cross-schema transaction. This backup job and a restore into a separate database must be demonstrated before claiming recovery; neither exists yet. Never overwrite the only surviving database during a drill. [Supabase backups](https://supabase.com/docs/guides/platform/backups).
- **Runner/image rollback:** retain the prior image digest and compatible schema. Stop the new process, restore the prior image on the same volume, and verify readiness/pairing. Do not downgrade the database blindly. A profile-format change may require an approved volume restore and fresh pairing.
- **Lost volume/host:** stop/fence the old runner, restore a new encrypted volume from a known snapshot, attach it to exactly one replacement, restore matching owner/profile references as necessary, and verify via Codex account status. OAuth refresh rotation can invalidate restored credentials; reconnect through the normal device flow if needed. Never hand-edit tokens. Restore operations must not overwrite the sole surviving volume.
- **Data loss targets:** daily profile snapshots imply up to 24 hours of new pairings/profile changes may be lost. The planned daily logical dumps have up to 24 hours of database data loss; until that backup job and restore are verified, there is no measured database recovery guarantee. Target manual alpha recovery within one hour, but measure it in the restore drill before claiming it.
- **Sprites and artifacts:** keep the same persisted Sprite resource ID across runner updates. Sprite storage is separate from profile storage and Postgres. Save provider checkpoints before destructive computer changes. Durable downloaded results are Postgres snapshots; restoring only the profile volume does not restore work history or files.

## Acceptance after deployment approval

Record deployment image digest, Machine/volume IDs, database migration level, masked owner/profile identifiers, bot/computer/run/routine IDs, UTC timestamps, downloaded artifact hash, and observed results. Never record credentials or device codes in the proof report.

1. Sign into the hosted Elsewhere URL with Drew's real account. Create a bot and its own computer through the product; allow at most the one new Sprite listed above.
2. Use **Connect ChatGPT** and let Drew complete Codex's device authorization. Confirm owner-scoped status, `gpt-5.6-luna`, and subscription-only execution. Do not reuse the operator's personal Codex profile.
3. Restart the runner with its volume retained, sign in from a fresh browser session, and confirm Codex still reports that owner's ChatGPT connection. Run a real subscription assignment after restart; a directory existing is insufficient proof.
4. Delegate one observable file-producing command with a roughly 40-second delay, which fits the existing Sprite exec timeout of 60 seconds. It must create a unique sentinel and a deliverable in `/workspace/results/{run-id}` using `AgentComputer`. Approve that exact command before leaving. Then stop local Elsewhere processes and disable the laptop's network/close it. Record an offline timestamp earlier than hosted completion.
5. From a phone or another computer/browser session, inspect durable progress and completion, download the result, and verify bytes/hash and the timestamp. Use a follow-up read-only assignment to inspect the same sentinel on the same Sprite after another runner restart.
6. Schedule a routine at least 15 minutes ahead to read that sentinel and produce a saved textual summary. Leave the laptop offline throughout its due time. Verify the scheduled occurrence and saved result later, then pause the test routine. Read-only tools need no new mutation approval. An unattended routine that requests a write or command must wait for approval or time out; the acceptance test must not disable the gate to force a green result.
7. Exercise graceful draining, queued-work recovery, forced interruption, duplicate-runner rejection, and a profile snapshot restore drill. Inspect any possibly completed remote actions before delegating a retry.

The 40-second command is sufficient only if the recorded offline timestamp precedes completion. If time is insufficient, repeat the bounded test; do not lengthen commands past existing limits or call a disconnected browser proof a laptop-off proof. Browser capability expansion stays deferred until work and routine gates are both green.

## Current evidence and approval boundary

No hosted ChatGPT pairing, restart persistence, laptop-off work/routine execution, live Sprite persistence, hosted artifact download, or volume recovery has been demonstrated in this phase. Local regressions are separate evidence, documented in `PRODUCT_PROGRESS.md` after verification.

The user approved Supabase Free and selected its organization; that database is now created and application connectivity is verified. The remaining proposal is the two Fly Machines, one 3 GB encrypted volume, shared Fly-provided HTTPS endpoints, and at most one acceptance Sprite, targeting **$14.88/month fixed / under $20 with light usage**. No Fly Machine, volume, new Sprite, paid database plan, or DNS change has been created in this database setup step. Confirm the remaining paid Fly deployment scope before provisioning; then complete image/preflight validation and live acceptance. Capacity or pricing outside this envelope and destructive recovery require another decision.
