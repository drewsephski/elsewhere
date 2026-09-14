# Background work operations

The cloud host is both the authenticated API and a supervised background runner. It must run on an always-on trusted machine to continue after the user's laptop closes. A development process on the laptop does not provide that guarantee.

## Durable admission

`POST /v1/runs` for a product account saves a queued assignment before returning 202. Use a stable `Idempotency-Key` for retries of the same submission. The queue snapshots the task, bot instructions, model, engine preference, and assigned computer. Later settings changes apply to new work. No browser connection is required for dispatch. Admission is bounded to 100 queued/active assignments per owner.

One worker per database is intentionally supported. It holds a dedicated Postgres advisory lock for its lifetime, checks that connection every second, and stops if the heartbeat fails. A second host fails startup before recovery. Run multiple concurrent assignments via `ELSEWHERE_MAX_CONCURRENT_RUNS`, not multiple independent hosts. Mount the same persistent private Codex profile volume after a restart or migration; profile references alone cannot restore credentials. This is a single-host supervision design, not distributed failover or an exactly-once side-effect guarantee.

Within the queue, a bot and a computer run one task at a time. A task on a different computer can run concurrently. The internal-token development bootstrap remains a legacy immediate-execution test path; product traffic must use JWT auth.

## Restart and cancellation

On startup after acquiring leadership, active work is marked interrupted and stale approvals are cancelled. Untouched queued work is retained and dispatched. Interrupted assignments are never replayed automatically because a terminal command or external operation may already have taken effect. The user can inspect saved results and progress and delegate a follow-up.

Cancellation of queued work is atomic with dispatch. Cancellation of active work sets a durable flag and signals the runtime; an already executing external operation may finish. Tool approval gates stay in place. Timeouts and runtime panics finalize work visibly and close outstanding approvals.

## Progress and troubleshooting

`GET /v1/runs/:id/events` replays durable events in batches and accepts `Last-Event-ID`. Closing the stream does not cancel the assignment. Database read failures end the stream so clients can reconnect. Work details and results remain available via the owner-scoped API and `/app/work/:id`.

If queued work does not start: confirm the cloud host is alive, Postgres is reachable, no other runner holds leadership, the bot/computer is not already working, and the configured concurrency is nonzero. If subscription work fails: verify the owner's ChatGPT connection and the private profile volume. Elsewhere never substitutes paid API usage.

Local verification uses an isolated test database. No live subscription authorization, Sprite provisioning, infrastructure deployment, or paid model call is required for the regression suite.

## Routines

`/app/routines` stores a bot, assignment, first start time, and fixed repeat interval (15 minutes to 30 days). The browser displays local time and sends an absolute timestamp. Intervals are elapsed time, not timezone-based cron: an every-24-hours routine can shift local clock time across daylight saving changes.

The runner checks due routines and commits the new Work item and next occurrence in the same database transaction. Concurrent scheduler ticks cannot duplicate an occurrence. Missed occurrences coalesce into one assignment; a routine with queued/active work advances its schedule without adding overlap. A failed or interrupted previous assignment pauses scheduling until explicit resume or a new manual attempt. Resume acknowledges the previous failed assignment; it does not replay it.

API: `GET/POST /v1/routines`, `PUT /v1/routines/:id`, `POST /v1/routines/:id/enabled`, and `POST /v1/routines/:id/run`. All are owner-scoped. Run-once accepts an `Idempotency-Key`, works while paused, and does not change the saved schedule. Pausing prevents future occurrences but does not cancel previously accepted work; stop that assignment from its Work page.

## Bot context and results

Migration 008 adds explicitly editable bot context with revision checks. Owners can read/write `/v1/bots/{id}/context`; a stale revision returns conflict without overwriting another edit. Context is limited to 16,000 bytes and is snapshotted together with the bot instructions when work is admitted. It applies to future manual and scheduled assignments. Context does not authorize actions or contain provider credentials.

Every assignment receives a dedicated `/workspace/results/{run-id}` output location. On completion, the host saves a summary and captures file bytes through `AgentComputer` only. Downloads are immutable Postgres snapshots with owner checks, `no-store`, `nosniff`, and attachment disposition. The Results page and work detail expose them without needing a running computer. Collection is limited to 30 seconds, 20 top-level files, 1 MiB per file, and 5 MiB total; folders, hidden/unsafe names, and unreadable/oversized files are skipped with a visible note. No collection writes or commands bypass approvals. The Sprite response reader bounds incoming success/error bodies to 16 MiB before accumulation.

An execution lease reserves the computer until result collection and cleanup finish, even if model execution has completed. Restart releases abandoned leases and marks possibly incomplete collection. Collection failure never causes automatic replay or changes a completed model assignment into failed work. Partial/interrupted work retains its assistant text and workspace files, but only completed assignments are collected into downloadable results.

Operational limit: snapshots currently live in Postgres and have no retention automation or object-storage tier. Back up the database; monitor storage growth before a public launch. Saved context is explicit user memory, not automatic extraction from chats. File capture is a snapshot of the run's output directory, not an attestation of file provenance or content safety.

## Readiness and deployment boundary

`GET /health` is process liveness. `GET /ready` requires a database response and a dispatcher heartbeat within ten seconds; it returns 503 before startup finishes or when scheduling/dispatch stalls. `/v1/workspace` gives owner-scoped counts and bot presence with the same readiness signal. Presence is derived from durable work and approval state, not a claim that a model session is currently connected.

To operate while the user's laptop is off, host the API/runner on an always-on machine. Run **one supervised cloud-host per database** with restart-on-failure, a stable persistent `ELSEWHERE_CODEX_PROFILES_DIR` outside all computer workspaces, and database backups. Use JWT-only authentication, HTTPS, correctly matched Better Auth issuer/audience/JWKS and web origin, and subscription mode. Keep provider secrets exclusively in the trusted host environment. Do not use ephemeral serverless processes for the runner or discard its profile volume on deploy. Use `/ready` for traffic/operational checks and structured host logs for failures, result-collection warnings, and recovery events.

No deployment, paid resources, DNS changes, live ChatGPT login, or provider calls were performed during the September product build. Local QA runs against separate test databases with an unavailable provider and unreachable Sprite endpoint. A stopped development laptop still stops a locally hosted runner; persistent computers alone do not provide always-on orchestration.

Bot settings apply only to new work. Changing the assigned computer does not transfer files. Concurrent partial settings updates preserve unrelated fields. The browser obtains authorization from the current Better Auth session for each request, avoiding stale-owner bearer reuse after account switching; existing JWTs remain subject to the server's configured expiration policy.

Web downloads use the same-origin `/api/results/:id/download` route. It obtains a bearer for the current Better Auth session server-side and streams the owner-scoped attachment from the runner with no caching or redirects. Browser download links never carry a bearer token in the URL.
