# Elsewhere → Grokbot-equivalent hosted agent platform

**This document is a dated assessment, not live product status.**

**Original assessment date:** 2026-09-18 (UTC)  
**Original baseline commit:** `997526f1145e1106087ffc2c00a1f53b93f9bae6`

**Current-status overlay (September 2026, after `final/web-readiness`):**

- Persistent team loop: `bot_list` → `bot_create` → `bot_delegate` with `onComplete: resume_source` → exactly one source continuation. Exposed on Responses and Computer MCP from one canonical collaboration schema.
- GitHub coding now includes a first-class PR publish/update loop (`github_publish_pull_request`, `github_update_pull_request`, certified check + approval). The Sep 18 “read-only GitHub / no PR loop” row is historical.
- Desktop CloudShell route parity has continued past the Sep 18 connectors/channels omission; verify `cloud-shell.tsx` / `app-routes.ts` rather than this table.
- Artifact handoff writes through `ComputerRegistry::connect_agent_computer` (Sprite and local Mac). Claimed transfers become terminal or reclaimable; they must not stay `transferring`.
- Hosted laptop-off / live ChatGPT pairing gates in `HOSTED_ALPHA_ACCEPTANCE.md` remain operator/manual proof, not automatically closed by this cleanup.

---

**Grokbot-equivalent bar (this roadmap):** Create a named Bot, assign a computer, give a meaningful job, leave (laptop closed), return to completed work with clear history and artifacts; reliable computer visibility and human takeover; approvals that are trustworthy and not bypassable; at least one solid repo-connected coding loop; routines/event triggers that fire work; web or desktop client usable for the core loop.

---

## A. Confirmed baseline

| Item | Status | Evidence |
| --- | --- | --- |
| **Exact SHA** | `997526f1145e1106087ffc2c00a1f53b93f9bae6` | `git log -1 origin/main` |
| **CI on this SHA** | **IN PROGRESS** at assessment time | [Actions run 35347037483](https://github.com/drewsephski/elsewhere/actions/runs/35347037483): `frontend` and `rust` jobs **success**; `cloud-host` job still running (`cargo test -p cloud-host --features test-utils -- --test-threads=1`). Prior main commit CI **success** (run 35344675237). |
| **Hosted alpha (Fly + Supabase)** | **Deployed** (Sep 14 inventory); **not accepted** | `docs/HOSTED_ALPHA_ACCEPTANCE.md`, `infra/fly/README.md` — web `https://elsewhere-alpha-web.fly.dev`, runner `elsewhere-alpha-runner`, Supabase `Elsewhere Alpha`. Pass/fail table: hosted alpha **not accepted**; many gates still **Fail/Pending** (BFF smoke, laptop-off, recovery drills, latest deploy of preview-cache/bootstrap fixes). |
| **Repo vs production** | **UNVERIFIED** whether Fly images match `997526f` | Acceptance doc last deploy evidence is **2026-09-14**; rows still say “Pending deploy” for browser bootstrap v3 + preview cache. Treat **latest `main` as ahead of likely production** until operator redeploys. |
| **Regression depth (local/CI)** | Strong | ~40 `cloud-host` integration test files, ~373 `async fn` tests in that tree; `pnpm test` + workspace builds in CI; portable crates (`agent-core`, `computer-mcp`, `codex-provider`, `sprite-computer`) tested in CI. |
| **Sep 17 Slack CI flake** | **Fixed on `main`** | `services/cloud-host/tests/channels.rs` — schema-based secret exclusion (`assert_slack_oauth_complete_response_excludes_secrets`) replaces brittle `"T1"` substring ban described in prior analysis. |

**Inference (labeled):** Sep 17 authenticated alpha observations (`codex_not_authenticated`, disabled preview) remain **plausible operational failures** but were **not re-run** in this assessment; engine failure string is confirmed in code (`run_engine_select.rs` → `codex_not_authenticated`).

---

## B. Architecture map (short)

End-to-end path for a **hosted Bot run**:

```text
User (Next.js apps/www or Tauri CloudShell)
  → Better Auth session (JWT)
  → BFF `/api/cloud/*` (apps/www/lib/cloud-host-upstream.ts)
  → cloud-host HTTP API (services/cloud-host, Axum)
  → Postgres (migrations: bots, agent_runs, work_queue, approvals, routines, …)
  → Single supervised runner (worker.rs: advisory lock, 1 Hz tick)
       → routines::tick / group_router / channels admission
       → work::claim_next → run lifecycle
       → Codex app-server (codex-provider) + per-run computer MCP (computer-mcp)
       → AgentComputer implementation:
            • fly_sprite → sprite-computer (Fly Sprite API, workspace on remote VM)
            • local_mac  → WebSocket RPC to Tauri companion (local_mac/session.rs)
  → Durable run_events (SSE replay), approvals gate, human_intervention, results finalization
  → Optional outbound: Slack delivery (channels/), webhook-triggered routines
```

**Profile / subscription boundary:** Owner-scoped Codex profiles on runner volume (`provider_profile.rs`, `ELSEWHERE_CODEX_PROFILES_DIR`); ChatGPT device pairing via providers API — credentials stay in Codex profile dirs, not Postgres.

---

## C. Capability matrix

| Area | Status | Evidence (paths / tests) | Gap vs Grokbot-equivalent |
| --- | --- | --- | --- |
| **1. Bot creation & configuration** | **WORKING** | `api/bots.rs`, `bot_context`, `skills/`, `permission_policies`, `memories`; UI `apps/www/app/app/bots`, settings; `durable_work.rs`, `skills.rs`, `permission_policies.rs`, `memories.rs` tests | Desktop shell omits some nav routes (see §7). No marketplace skills — intentional. |
| **2. Run execution engine** | **PARTIAL** | `work.rs`, `worker.rs`, `run_lifecycle.rs`, `codex-provider`; `durable_work.rs`, `approval_lifecycle.rs`, `assistant_stream_replay.rs`; `docs/BACKGROUND_WORK.md` | Code + regressions strong; **hosted laptop-off completion not proven** (`HOSTED_ALPHA_ACCEPTANCE.md` gates Fail). Fail-closed without ChatGPT pairing (`codex_not_authenticated`). |
| **3. Computer / environment** | **PARTIAL** | Sprite: `crates/sprite-computer`, `computer_registry_lifecycle.rs`; Mac: `migrations/038_local_mac.sql`, `local_mac_pairing.rs`, `local_mac_bot_run_routing.rs`; preview: `browser-preview-*`, `computer_control.rs`, `human_intervention.rs` | Provisioning/readiness fragile on alpha (docs + prior live notes). **Live preview + Take Control implemented in repo** but hosted E2E gates pending. No user-facing “reset/recover computer” UX. |
| **4. Repo & workspace integration** | **PARTIAL (dated Sep 18)** | Workspace tools via MCP/computer exec; attachments `attachments.rs`; GitHub tools in `connectors/` and `github_coding/` | **Historical gap:** this assessment predates the certified GitHub PR publish/update loop. Check current `GITHUB_*_TOOL` catalog and `github_coding` tests. |
| **5. Approval / tooling flow** | **WORKING** | `approval/gate.rs`, `human_control_gate.rs`, `permission_policies.rs`; `approval_lifecycle.rs`, `approvals.rs`, `ask_user.rs`; `AmbiguousOutcome` in `agent-core/computer.rs`, `local_mac/session.rs` (no auto-retry) | “Auto Review” style automation **MISSING** (human gate by design). MCP + connector tools behind same gate (`computer-mcp/tools.rs`). |
| **6. Hosted web UI** | **WORKING** | Full App Router shell: work, approvals, results, routines, computers, connectors, channels, skills, groups (`app-routes.ts`) | Polish/gates ≠ Grokbot trust yet. |
| **7. Desktop parity** | **PARTIAL (dated Sep 18)** | Tauri `src/main.tsx` → `CloudShell` reuses `apps/www` via Vite aliases | **Historical:** this assessment noted omitted connectors/channels routes. Re-check `cloud-shell.tsx` before treating that as current. |
| **Routines (cron + webhook)** | **WORKING** | `routines.rs`, `routine_webhooks.rs`, `schedule.rs`; UI `apps/www/app/app/routines` | Hosted fire-while-away **unproven** (acceptance gate). |
| **Channels (Slack)** | **PARTIAL** | `channels/slack/*`, `channels.rs` tests, `apps/www/app/app/channels`; delivery + admission in `worker.rs` | Live acceptance **manual only** (`slack_acceptance.rs` `#[ignore]`). OAuth refactor on tip commit — CI pending. |
| **Multi-bot / delegation** | **WORKING (dated overlay)** | `bot_create.rs`, `delegation.rs`, `collaboration_lifecycle.rs`, `artifact_handoff.rs`, group tests | `bot_create` + `resume_source` + MCP schema parity are current. Remaining gap is hosted manual proof of the team loop, not missing primitives. |
| **Onboarding / pairing** | **PARTIAL** | Invite signup, ChatGPT device flow (`api/providers.rs`), Mac pairing (`api/local_mac.rs`, `pair-mac-view.tsx`) | Multi-step (ChatGPT + computer + optional Slack); failure UX when Codex unavailable is accurate but harsh for “just works”. |
| **Recovery UX** | **MISSING** (product) | Operator runbooks: `infra/fly/README.md`, `HOSTED_ALPHA_ACCEPTANCE.md` (DB/volume restore gates) | No in-app “update / recover / reset computer” like Grokbot settings. |
| **Observability** | **PARTIAL** | Structured phase logging target `elsewhere_run_phases` in `runner.rs`; run timeline in UI | No owner-facing SLO dashboard; latency gates **Fail** in acceptance doc. |

---

## D. What’s already strong

1. **Durable work architecture** — Admission idempotency, bot/computer serialization, cancel/interrupted semantics, SSE replay, and single-runner leadership are implemented and heavily tested (`durable_work.rs`, `worker.rs`, `assistant_stream_replay.rs`).
2. **Security posture** — Owner-scoped APIs, IDOR tests (`idor.rs`), encrypted connector tokens, Slack OAuth response schema tests, JWT modes, RLS on Supabase (documented in `PRODUCT_PROGRESS.md`).
3. **Approval + human control** — Lifecycle tests cover races, restart recovery, and take-control without silently resolving interventions (`human_intervention.rs`, `computer_control.rs`).
4. **Product surface area** — Routines (schedule + webhook), results/immutable downloads, skills snapshots, bot memory, groups/@mentions, delegation with resume, browser preview cache with transactional consistency (`HOSTED_ALPHA_ACCEPTANCE.md` closure pass).
5. **Computer abstraction** — Shared `AgentComputer` trait across Sprite and local Mac; `AmbiguousOutcome` prevents unsafe retries on Mac RPC drops.
6. **Desktop strategy (directionally right)** — One web component tree reused in Tauri CloudShell reduces drift vs maintaining two UIs — parity gaps are routing omissions, not greenfield UI work.

---

## E. Critical blockers (ordered)

1. **Hosted operational proof (laptop-off + fresh session)** — Without a documented, repeatable pass on Fly, the core Grokbot promise is **story-only** (`HOSTED_ALPHA_ACCEPTANCE.md` still **No**).
2. **ChatGPT/Codex + computer readiness on the happy path** — Work stuck on `codex_not_authenticated` or “computer starting” blocks everything; pairing and Sprite bootstrap must be **boring** before connectors matter.
3. **Deploy drift** — Acceptance checklist still pending redeploy for preview cache, browser profile dirs, bootstrap v3; production may lag `main`.
4. **Live computer visibility E2E** — Code exists (`ComputerBrowserPreview`, human control hooks); hosted gates for preview-after-tools and Luna browser acceptance are **Fail/Pending**.
5. **Repo-connected coding loop** — **Historical as of this cleanup.** GitHub coding tools now include publish/update PR; remaining work is hosted/manual QA of the same-PR revision loop, not a missing tool.
6. **Trust hardening** — Public runner ingress still enabled until BFF smoke passes (`HOSTED_ALPHA_ACCEPTANCE.md`); recovery drills (DB + volume + drain) incomplete.
7. **Desktop parity gaps** — Missing connectors/channels routes in `cloud-shell.tsx` for a single-client story.

---

## F. Prioritized roadmap (sequenced)

### P0 — Close the core loop on hosted alpha (trust)

**P0.1 — Redeploy + BFF smoke + ingress hardening**  
- **Goal:** Running Fly images match current `main`; all product API traffic via web BFF; runner not publicly exposed without intent.  
- **Why first:** Every other gate depends on an honest production-like stack.  
- **Work:** `infra/fly/*`, `apps/www/lib/cloud-host-upstream.ts`, Fly secrets/topology per `infra/fly/README.md`; execute rows in `HOSTED_ALPHA_ACCEPTANCE.md` (BFF smoke, remove public runner ingress, re-smoke).  
- **DoD:** Acceptance table rows “Deploy + hosted BFF smoke” and “Public runner removed + re-smoke” **Pass**; `/ready` healthy on runner; anonymous cloud-host **401**.  
- **Size:** **M**

**P0.2 — ChatGPT pairing + Codex gate UX**  
- **Goal:** New owner can connect ChatGPT once; work fails with **actionable** UI (link to Connect) until ready; no silent queue rot.  
- **Why:** Unblocks `codex_not_authenticated` class failures.  
- **Work:** `api/providers.rs`, `run_engine_select.rs`, workspace overview/bot chat error surfaces in `apps/www`.  
- **DoD:** Scripted checklist: create bot → connect ChatGPT on hosted runner → assignment reaches **running** → **completed** with live Codex (not Responses fallback).  
- **Size:** **S**

**P0.3 — Computer provisioning happy path (Sprite)**  
- **Goal:** Assign cloud computer → `providerMetadata.provisioned` true → workspace usable without operator intervention.  
- **Why:** Execution requires `ensure_ready` on Sprite (`sprite-computer/src/computer.rs`).  
- **Work:** `computer_registry.rs`, Sprite bootstrap/versioning, computers UI (`computers-manager.tsx`).  
- **DoD:** Acceptance Sprite passes Luna write/read sentinel (already partially proven Sep 14) **from a cold owner account** without manual Fly steps.  
- **Size:** **M**

**P0.4 — Laptop-off proof (assignment + routine)**  
- **Goal:** Documented R2-style proof completes while user offline (`HOSTED_ALPHA_ACCEPTANCE.md` checklist).  
- **Why:** Defines “Grokbot-equivalent” for this product.  
- **Work:** Operator procedure + minor fixes from failures only.  
- **DoD:** Assignment **completed** and webhook/cron routine occurrence **completed** while laptop closed; fresh browser session shows history + artifact download.  
- **Size:** **S** (engineering) / **M** (calendar coordination)

---

### P1 — Visibility, control, and coding credibility

**P1.1 — “Open computer” E2E on active work**  
- **Goal:** Reliable live browser preview during runs + expand/takeover path on work detail and bot workspace.  
- **Why:** Primary Grokbot feel; forces Sprite browser daemon + cache path to work hosted.  
- **Work:** `browser-preview-view.tsx`, `use-browser-human-control.ts`, `computer_control.rs`, Sprite guest browser assets.  
- **DoD:** Acceptance gates “Preview after tool results” and “Luna browser acceptance” **Pass**; human can take control and return bot without bypassing approvals.  
- **Size:** **M**

**P1.2 — One repo-connected coding loop (minimal)**  
- **Goal:** Owner connects GitHub; bot clones/edits/pushes via **computer git** with clear result (branch or PR link in work summary).  
- **Why:** Grokbot-equivalent bar explicitly requires a solid coding loop; read-only GitHub tools alone insufficient.  
- **Work:** Documented skill or routine template + optional `github_create_branch` / PR API (if added, extend `connectors/service.rs` + tests — **only if product chooses API path**); otherwise certify terminal workflow with regression fixture on Sprite.  
- **DoD:** Repeatable demo: “Fix README typo in repo X” → visible diff on GitHub (PR or direct push per policy) + saved artifact in Results.  
- **Size:** **L** (API PR path) / **M** (documented git-on-Sprite path with test fixture)

**P1.3 — Slack channel hardening**  
- **Goal:** DM → durable run → approval notice → resume → reply; CI-stable OAuth.  
- **Why:** Real-world trigger channel; tip commit is Slack-focused.  
- **Work:** `channels/slack/*`, `apps/www/app/app/channels`, optional opt-in smoke test harness (not production Slack in CI).  
- **DoD:** Manual `slack_acceptance.rs` procedure passes once on staging; unit tests green on `main` CI.  
- **Size:** **M**

**P1.4 — Recovery drills (operator → product)**  
- **Goal:** Proven DB logical backup/restore + volume snapshot restore + Fly SIGTERM drain without duplicate side effects.  
- **Why:** Trustworthy approvals imply trustworthy recovery.  
- **Work:** `docs/SUPABASE_ALPHA.md`, Fly ops, surface **read-only** status in settings (no destructive buttons yet).  
- **DoD:** Acceptance recovery rows **Pass**; interrupted runs marked **interrupted**, not auto-replayed (`docs/BACKGROUND_WORK.md` semantics verified on Fly).  
- **Size:** **M**

---

### P2 — Parity and leverage

**P2.1 — Desktop CloudShell route parity**  
- **Goal:** Tauri app exposes connectors + channels + group routes matching web nav (`appShellNavLinks`).  
- **Why:** “Desktop or web client usable” — single codebase, small routing diff.  
- **Work:** `src/cloud-shell.tsx` add routes importing existing pages.  
- **DoD:** Desktop signed-in user can complete Connect GitHub + Connect Slack without browser.  
- **Size:** **S**

**P2.2 — Local Mac (`local_mac`) production path**  
- **Goal:** Pair This Mac → assign to bot → hosted run executes on Mac when online; clear offline behavior.  
- **Why:** Differentiator vs Grokbot shared cloud computer; code largely on `main` (`local_mac_bot_run_routing.rs`).  
- **Work:** Tauri `host_link.rs`, `local_mac/session.rs`, computers UI labels (`computer-kind.ts`).  
- **DoD:** Integration test scenario runs green + one manual Mac companion demo with laptop open; offline queues work with explicit status (no ambiguous retry).  
- **Size:** **L**

**P2.3 — Delegation polish**  
- **Goal:** Source bot resume on delegate completion is visible in UI; artifact handoff failures actionable.  
- **Why:** Multi-bot is scaffolded but resume logic now exists in backend.  
- **Work:** `collaboration_completion.rs`, work timeline UI, `artifact_handoff.rs`.  
- **DoD:** End-to-end delegate → complete → source bot continues without owner manually nudging; covered by extended `collaboration_lifecycle.rs`-style scenario.  
- **Size:** **M**

**P2.4 — Observability for operators**  
- **Goal:** Publish TTFT / phase breakdown from `elsewhere_run_phases` for text, browser, and follow-up turns.  
- **Why:** Prioritize perf work; avoid premature Codex pooling (`HOSTED_ALPHA_ACCEPTANCE.md`).  
- **Work:** `runner.rs` logging, small admin/readout page or documented log query.  
- **DoD:** Three measurements recorded in acceptance doc; regressions alert on dispatcher not alive.  
- **Size:** **S**

---

### P3 — Deliberately later (out of Grokbot-equivalent scope)

- Plugin/marketplace connectors beyond GitHub + Slack + generic webhook.  
- Mobile client, semantic search across results, arbitrary untrusted skill packages (`ROADMAP.md`).  
- Shared-computer multi-bot screen model (conflicts with Elsewhere 1:1 bot↔computer design).  
- Auto Review / fully automated approval policies.

---

## Corrections vs Sep 17 hypothesis

| Prior claim | Verified on `997526f` |
| --- | --- |
| Slack OAuth CI flake (`"T1"`) | **Fixed** — schema-based assertions in `channels.rs` |
| Delegation never wakes source bot | **Partially outdated** — `source_resume_run_id` + tests in `delegation.rs`, `collaboration_lifecycle.rs` |
| Desktop shell merged (#19–#21) | **Confirmed** — CloudShell pattern on `main`; not full nav parity |
| Live preview “disabled placeholder” only | **UI has full preview stack**; hosted **E2E still pending** in acceptance doc |
| PR #13 unmerged | **UNVERIFIED** — fix appears on `main`; do not rely on PR state |

---

## Recommended single next build (after P0.1 deploy)

**Ship P0.2 + P0.4 together:** make ChatGPT pairing impossible to miss, then execute the written laptop-off checklist on the acceptance bot/computer. That is the smallest path to an honest Grokbot-equivalent claim for Elsewhere’s differentiation (BYO ChatGPT + dedicated computers).
