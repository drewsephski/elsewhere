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
