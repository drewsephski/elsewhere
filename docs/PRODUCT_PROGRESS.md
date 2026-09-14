# Elsewhere product build — September 13–14, 2026

## Product assessment

Elsewhere is a place to create AI workers and give them computers. The implemented subscription engine, portable AgentComputer boundary, Postgres history, ownership, and approval gate are the foundation. The previous top-level docs substantially understated those capabilities.

The biggest gaps are continuity of execution, safe account pairing, recurring work, remembered context, and finding finished results. The dashboard exposes transport/provider vocabulary and gives little reassurance about what is happening when the user leaves.

Grok Bot's useful reference is its persistent named teammate, simple delegation, a computer with durable files, visible work, approval handoffs, and recurring workflows. See the [official overview](https://docs.x.ai/grok-bot/overview), accessed September 13. Elsewhere should differentiate through the user's ChatGPT/Codex allowance, explicit subscription billing boundaries, and separately assignable computers rather than copying Grok's shared-computer model.

## Milestones

1. **Private, persistent ChatGPT pairing.** Every product owner gets a stable host-only profile; supported device sign-in; subscription-only automatic selection; ownership regression coverage.
2. **Durable background work.** Persist admission, serialize use of a computer, recover queued work, make interrupted work explicit without replaying side effects, and replay progress across client reconnections.
3. **Routines.** Persist a simple schedule and task, execute through the same ownership/approval path, prevent overlapping or duplicate work, expose pause and history.
4. **Context and results.** Editable useful bot memory supplied to work, durable results and a clearer work-oriented dashboard.

Browser interaction will follow once these single-bot guarantees are dependable. It must remain behind AgentComputer and the existing approval gate. No multi-agent or infrastructure expansion is needed for these milestones.

## Checkpoints

### 1 — ChatGPT pairing

- Persistent Postgres owner-to-profile mapping; UUID directory names prevent identity/path confusion.
- Product login, status, and execution use the same private profile on a trusted runner volume. Unconfigured product profiles fail closed; they never use the operator's personal login.
- Device sign-in uses the installed Codex app-server's `chatgptDeviceCode` protocol. The UI shows the verification code and link, polls with bounded retries, and allows owner-scoped cancellation.
- Pending logins expire after ten minutes. Credentials are written/read/refreshed by Codex only. Elsewhere stores profile references, not credentials.
- Automatic selection never switches to paid Responses usage. Explicit Responses selection remains available.
- Provider validation precedes computer provisioning. Owner lookup failures no longer fall back to the trusted local principal.

Live device authorization requires the user's participation and has not been performed. A persistent runner deployment and volume are still required to remain available while the development laptop is off.

### 2 — Durable work and progress

- Product admission commits the assignment, immutable execution settings, conversation, and initial event in one transaction. Owner-scoped idempotency prevents duplicate tasks and rejects key reuse for different requests.
- A supervised single runner claims queued work from Postgres and serializes bots/computers. A dedicated database lock prevents another runner from interrupting live work during startup. Loss of that connection stops the host.
- Queued work survives restart; potentially executed work becomes interrupted and is not retried automatically. User cancellation is durable, including before dispatch. Active or queued computers cannot be archived.
- Progress streams read paginated durable events, support reconnect cursors, and remain open while work waits in the queue. They no longer depend on a live subscriber registry or silently drop overflow.
- Work has its own addressable page, persisted result, readable progress, inline approvals, cancellation, and bot history. Transport details and resource IDs are removed from the primary delegation flow.
- Bot chat reuses one primary Postgres conversation per bot. Follow-up sends share message history for the Responses engine and resume the same Codex app-server thread via `thread/resume`, with `thread/compact/start` after a bounded number of completed turns.
- A bot settings update no longer accidentally clears its computer assignment when the field is omitted.

Milestone 2 verification: cloud-host regression tests pass against local Postgres, including isolated queue/approval tests and 1,001-event cursor replay. Web and desktop type checks and desktop tests pass. Browser QA confirmed sign-in, computer/bot creation, durable work submission, an explicit disconnected-provider failure, and history after refresh. Live providers were deliberately disabled for browser QA.

### 3 — Scheduled routines

- Complete routine creation/editing, first-run time, fixed repeat intervals, pause/resume, and manual run-once in the web dashboard.
- Each occurrence uses the same durable work admission and approval path. Schedule advancement and admission commit atomically; stable occurrence keys prevent duplication.
- Missed time slots coalesce; active work does not overlap; failed/interrupted prior work pauses the routine with an actionable explanation. Explicit resume acknowledges the prior failure without replaying it.
- Owner-scoped APIs and regression cases cover concurrency, missed occurrences, cancellation/pause, manual idempotency, foreign ownership, unavailable computers, and resume after failure.

Milestone 3 verification: full cloud-host regressions and six routine-specific tests pass against Postgres; web type checks pass. Browser QA confirmed routine creation, pause, and run-once navigation to persisted work, with live providers disabled.

### 4 — Useful context and saved results

- Bot memory has an owner-editable, versioned surface with conflict handling and immutable snapshots for queued work and routines.
- Completed work now captures its summary and bounded output files through AgentComputer, preserving the approval boundary. Immutable downloads remain available independently of the computer.
- Added Results navigation, per-assignment saved files, and private attachment downloads. Computer execution stays reserved while files are collected.
- Tightened the Sprite client to reject oversized response bodies during reception, for both successful and failed requests.

Milestone 4 verification: full cloud-host and Sprite regressions pass, including context ownership/conflicts/snapshots, bounded file capture, immutable snapshots, execution reservation/restart, and private download headers. Browser QA confirmed context persistence after reload and populated Results views using clearly labeled local fixtures. The in-app browser did not emit a download event; authenticated download bytes and headers are verified by the HTTP regression. No live model or paid infrastructure is implied by this verification.

### 5 — Workspace presence and product usability

- Live, owner-scoped workspace counts and bot presence distinguish working, queued, waiting for approval, saving results, ready, and needs-attention states. Empty workspaces have concrete setup links.
- Bot creation centers on a teammate's name, role, and computer. Editable bot settings preserve queued execution snapshots; concurrent partial updates no longer overwrite unrelated fields.
- Improved asynchronous failure handling and duplicate-submit protection for computer and approval actions, with product-level language instead of transport/tool identifiers.
- Added dispatcher-backed readiness and a visible unavailable-runner state. Removed bearer caching across browser account changes.
- Updated the roadmap and cloud architecture to match the implemented product, with explicit deployment and live-provider proof gates.

Milestone 5 verification: 60 cloud-host tests and 55 portable runtime/provider/computer tests pass; desktop's 10 tests and both TypeScript checks pass. The production Next build succeeds. Cargo Clippy completes with existing warnings. Browser QA confirms live overview counts, bot-role updates, saved context, native authenticated result downloads, and a readable mobile layout without page overflow. The native download route also rejects anonymous access and returns exact saved fixture bytes for its owner.

Final local state: five clean product checkpoints; no push or deployment. The pre-existing change in `apps/www/next-env.d.ts` is preserved outside these commits. QA uses an isolated database and visibly labeled result fixtures. Real ChatGPT authorization, paid infrastructure, and browser/computer-use execution were not exercised.


### 6 — Hosted alpha proposal and shutdown preparation

The September 14 request makes live laptop-off execution the next gate. The user confirmed there are no hosted web/control-plane/Postgres resources to assume and asked for a Fly proposal **before provisioning or deployment**. See [the resource, cost, secrets, recovery, and acceptance proposal](HOSTED_ALPHA_PROPOSAL.md). The existing `elsewhere` Sprite remains a computer test resource.

Local preparation completed before the approval pause:

- Enabled SQLx TLS for a hosted Postgres connection. The proposal requires the direct database endpoint for the single-runner session lock.
- Acquire leadership before migrations/recovery. Handle SIGTERM and SIGINT with a 240-second drain: readiness becomes false, new dispatch stops, active work/artifact collection can finish, and the worker continues leadership checks and cancellation processing.
- Track execution tasks and abort/join them before relinquishing leadership. Drop also cancels the per-run MCP server so timeout/abort paths do not leave a computer endpoint listening.
- Added regressions for drain/queue preservation, execution shutdown, and MCP listener revocation. CI now includes web type generation/checking and the computer-MCP/Codex-provider suites.

Verification: **122 Rust tests passed**, including cloud-host tests against a separate local Postgres database; both TypeScript checks and 10 desktop tests passed. `cargo check -p cloud-host` and Clippy for cloud-host/computer-MCP all targets passed with existing warnings. A real local cloud-host process reached readiness, rejected a second runner, exited cleanly on SIGTERM, and reached readiness again after restart. `git diff --check` passed. Repository-wide `cargo fmt --all -- --check` reports pre-existing formatting differences; no broad formatting rewrite was made. Hosted Linux image, remote TLS connection, GitHub CI execution, and live provider behavior are not proven by these checks.

Still unproven: hosted ChatGPT device authorization; subscription execution/pairing across hosted restart; laptop-off work and routine completion; live Sprite persistence; hosted artifact retrieval; volume restore/recovery. No paid resources, deployment, DNS changes, OAuth token handling, API fallback, or browser feature expansion occurred. Docker/Fly deployment packaging and admission restrictions remain work after proposal approval.

### 7 — Supabase Free control plane

The user requested an under-$20 alternative, approved Supabase, and selected **drew's projects**. Created **Elsewhere Alpha** (`edbfhcveqxtxnfybhmej`, Free, us-east-2). The [revised proposal](HOSTED_ALPHA_PROPOSAL.md) replaces Fly Managed Postgres with Supabase and reduces the proposed web/volume sizes: **$14.88/month fixed**, with variable usage needing to remain within the $20 target. Fly authentication is restored, but paid Fly provisioning and deployment remain pending.

- Created separate private schemas and restricted runner/web roles. Enabled verified TLS and server-side SSL enforcement; disabled the unused Data API. Enabled RLS on all 18 application tables with no client policies. Codex credentials remain outside Postgres.
- Applied all nine SQLx migrations and the installed Better Auth 1.4.21 migration. Made the auth schema configurable, kept the local default, bounded its connection pool, and fixed initialization for a role without database-wide CREATE permission. Replaced the moving `npx ...@latest` migration command with the pinned CLI.
- Verified actual service-role TLS connections, rolled-back reads/writes, cross-schema access denial, and rejection of non-TLS connections. With RLS applied, two local runner processes successively reached readiness against hosted Postgres and exited cleanly on SIGTERM. The actual web database module passed schema initialization under its restricted role.
- Supabase advisors report no security warnings/errors; intentional owner-only RLS notices and informational performance findings are documented in [the database runbook](SUPABASE_ALPHA.md). Both TypeScript checks and all 10 existing frontend tests passed. Remote GitHub CI was not run in this step.

No Fly Machine/volume, new Sprite, paid plan, DNS change, or hosted web/runner deployment was created. No ChatGPT pairing, laptop-off work/routine, profile persistence, live Sprite/artifact retrieval, or backup restore is claimed. Supabase Free has no automatic backups; implementing and proving the documented backup/restore path remains required before inviting users.

### 8 — Hosted web and persistent runner deployed

The user approved the reduced testing scope. Deployed one 512 MB Fly web Machine (autostop enabled) and one always-on 2 GB runner, backed by Supabase Free and a single encrypted 3 GB profile volume with seven-day scheduled snapshots. See [the live inventory and acceptance report](HOSTED_ALPHA_ACCEPTANCE.md) and [operations](../infra/fly/README.md).

- The user created their real hosted Elsewhere account. Invitation-gated signup rejects missing/invalid codes before password hashing and covers all user-creation hooks. The hosted workspace loads owner-scoped data through JWT authentication.
- Linux image validation revealed that Codex's app-server schema omits CLI configuration fields. Fixed compatibility verification to use actual MCP config parsing, confirm the returned tool allow-list, and reject an invalid exposure negative control. Removed reliance on a help command that can skip configuration validation. Codex remains pinned to 0.154.0.
- Linux preflight passed protocol/device schema and a disconnected app-server handshake, missing-mount rejection, duplicate-runner exclusion, clean SIGTERM, and mounted sentinel persistence across restart. No real credentials were involved in these local checks.
- Hosted `/ready` returned healthy database/dispatcher state; anonymous API requests returned 401. Actual Machine configuration confirms one runner, the encrypted mount, restart-always, and 300-second SIGTERM grace. After stopping the web Machine, a JWKS request woke it successfully in 8.91 seconds.
- Verification: 30 Codex-provider Rust tests, 12 frontend tests, both TypeScript/lint commands, Clippy with existing warnings, Linux builds, and Fly config validation passed. GitHub CI has not been run remotely.

The user completed real hosted ChatGPT device pairing. The connection persisted after a hosted runner restart and full browser reload. A real five-tool Luna assignment completed through approved Sprite writes and read-back, and saved an artifact that triggered a browser download. The same Machine/volume survived an image update; a profile snapshot completed. Local web/Postgres are now stopped. A timed assignment and 04:40 Central routine are prepared for the user to close the laptop. Actual laptop-off completion, fresh-session retrieval, and backup restore remain open gates. These deployment checks do not establish the laptop-off promise.
