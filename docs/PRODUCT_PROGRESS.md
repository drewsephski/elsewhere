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
