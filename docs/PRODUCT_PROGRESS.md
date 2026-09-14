# Elsewhere product build — September 13, 2026

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
