# Grok Bot vs Elsewhere — gap analysis

**Date:** September 15, 2026  
**Grok Bot source:** Product capability brief (`uploads/GROKBOT_CAPABILITY_BRIEF.md`) — not private Cursor source.  
**Elsewhere source:** This repository’s code, migrations, and docs (`docs/PRODUCT_PROGRESS.md`, `docs/ROADMAP.md`, `docs/ARCHITECTURE.md`, etc.).

**Primary question:** What does Grok Bot have that Elsewhere does not (yet)?

---

## Elsewhere architecture (shipped shape)

Elsewhere is a **web-first** product (`apps/www`, Next.js + Better Auth) backed by a **Rust control plane** (`services/cloud-host`) on Postgres. Execution uses a portable agent loop (`crates/agent-core`) with two run engines: **Codex** (ChatGPT subscription, default) and **Responses** (explicit API key). All computer access goes through **`AgentComputer`**:

| Adapter | Path | Role |
|--------|------|------|
| `SpriteComputer` | `crates/sprite-computer/` | Fly Sprites: `/workspace` files, exec, headless browser (Playwright guest) |
| `LocalMacComputer` | `src-tauri/src/agent/local_mac_computer.rs` | macOS Virtualization.framework + `guest-agent` (desktop experiment) |

Tools reach the model via **Elsewhere MCP** (`crates/computer-mcp/`) bridged per run from `services/cloud-host/src/runner.rs`. Durable work, SSE replay, approvals, routines, context snapshots, and results live in Postgres (migrations `001`–`027+`). A **single supervised runner** holds a DB advisory lock (`docs/BACKGROUND_WORK.md`). Hosted alpha: Fly web + always-on runner, Supabase Postgres, encrypted Codex profile volume (`docs/HOSTED_ALPHA_ACCEPTANCE.md`, `infra/fly/README.md`).

Desktop (`src-tauri/`) reuses workspace UI via Vite proxy; production path is hosted web + cloud runner, not a Grok-style desktop shell as primary.

---

## Capability matrix

| Capability | Elsewhere (status + evidence) | Grok Bot (brief) | Gap? |
|------------|-------------------------------|------------------|------|
| **Persistent named bots** | **Shipped:** `bots` table, `POST/PATCH /v1/bots`, `create-bot-dialog.tsx`, name/role/instructions/computer/avatar at create (`migrations/011_bot_avatar.sql`, `bot_avatar.rs`) | Profile, avatar, title, description | Minor: no separate “title” field; avatar not in bot settings after create |
| **Per-bot computer** | **Shipped:** One `computer_id` per bot; separate Fly Sprites (`sandboxes`, `SpriteComputer`) | Per-bot “box”; shared Linux FS across user’s agents | **Different model:** Elsewhere assigns **distinct** computers; Grok shares FS with per-agent desktop windows |
| **Shared user computer / multi-bot FS** | **Absent** as product model (`docs/ROADMAP.md` deliberately separate computers) | One shared Linux FS across agents | Yes — intentional product difference |
| **Desktop chat app** | **Partial:** Tauri shell exists; primary UX is `apps/www` | Desktop app chat UI, multi-bot sidebar | Yes — Grok is desktop-native; Elsewhere is web-primary |
| **Hidden chats** | **Absent:** no hidden conversation flag; run archive only (`run_archive.rs`) | Hidden chats | Yes |
| **Group channels of bots** | **Partial:** `groups.rs`, `/v1/conversations/groups`, `group-conversation-view.tsx`, `group_router.rs`; limits in `PRODUCT_PROGRESS.md` §9 | Group channels, dedicated group skill | Yes — Grok more mature; Elsewhere lacks parallel multi-bot execution on one host |
| **Local machine registration (Mac shell)** | **Partial (desktop only):** `LocalMacComputer` + VM/guest-agent; **not** hosted product path | Register Mac for approved local Shell/Read/transfer | Yes — no Grok-style “target my Mac” from cloud product |
| **Act-first autonomy / subagents** | **Absent:** no `computerUse`, `generalPurpose`, or `videoReview` subagent types in repo | Background subagents + todo/resume/stop | **Yes** |
| **Cloud coding agents (repo/PR)** | **Absent:** no Cursor cloud-agent integration, branch/PR automation | Cloud agents for GitHub/GitLab/Bitbucket/Azure DevOps/Origin | **Yes** |
| **Cursor IDE / plugin marketplace** | **Absent** in Elsewhere product | Deep Cursor integration; Convex/Prisma/GitHub/Gmail/Neon plugins | **Yes** |
| **MCP connector catalog** | **Partial:** fixed Elsewhere MCP + optional GitHub OAuth connector (`owner_connectors`, `connectors/`, `connector_tools.rs`); no user-installed MCP servers | Install/auth marketplace plugins in-place | **Yes** |
| **Dynamic MCP discovery** | **Partial:** Codex thread MCP config tests (`mcp_exposure_config.rs`); not user-driven catalog | Dynamic namespace discovery | Yes |
| **Skills / playbooks** | **Shipped (owner skills):** `crates/agent-skills/`, `skills`/`bot_skills`/`run_skills` (`025_product_skills.sql`), “Save as skill” on work (`skills-manager.tsx`, `work-detail.tsx`) | Managed + user + **plugin** skills; forced read-before-use | Yes — no plugin skill packs; no learn-from-demo → skill |
| **Domain playbooks** | **Absent** (food, rides, Resy, DoorDash, etc.) | Many site-specific playbooks | **Yes** |
| **Routines — time schedules** | **Shipped:** `routines`, `schedule.rs`, fixed intervals + daily/weekly + timezone (`022_routine_schedules.sql`), UI `routines-manager.tsx` | Cron schedules | Partial parity |
| **Routines — event listeners** | **Absent:** no Slack/GitHub/Linear/Sentry/PagerDuty/webhook triggers in `cloud-host` | Event-driven routines | **Yes** |
| **Memory tiers (profile/log/note)** | **Absent:** single owner-editable `bot_context` (`008_context_results.sql`, `bot-context.tsx`); docs deny auto-extraction (`ROADMAP.md`) | Durable tiers + agent/shared user facts | **Yes** |
| **Teammate messaging** | **Partial:** `bot_list`, `bot_delegate` (`collaboration_tools.rs`, `014_bot_delegations.sql`); no `SendToAgent`, no wake originator on complete (`PRODUCT_PROGRESS.md` §9) | SendToAgent, CreateAgent, UpdateAgent, CreateChannel | Yes |
| **Approvals** | **Shipped:** `tool_approval_requests`, `approval-card.tsx`, read auto-allow + mutation gate (`approval/gate.rs`) | Auto-review gate on risky Shell/MCP/computer/cloud | Partial — Grok has broader “auto-review” product surface |
| **In-chat forms / widgets** | **Absent** in chat | `request_user_form`, question widgets | **Yes** |
| **Voice memos** | **Absent:** desktop mic disabled (`chat-composer.tsx`) | Spoken replies | **Yes** |
| **Attachments in chat** | **Partial:** desktop “Add attachment” disabled; web chat no upload path found | Attachments, screenshots, generated images | Yes |
| **Browser / GUI automation** | **Shipped (Sprite path):** `browser_*` tools, `docs/BROWSER.md`, human takeover `computer_control_leases` (`027_*`), preview API | `computerUse` subagent for desktop/browser | Partial — Elsewhere has structured browser tools + owner control; Grok emphasizes autonomous GUI subagent |
| **CopyToBox / CopyFromBox** | **Absent** as named product; workspace tools + partial delegation artifact transfer (`artifact_handoff.rs`) | Explicit box ↔ user machine copy | Yes |
| **Outside channels (Slack inbound)** | **Absent** (spec only `PRODUCT_SPEC.md` §18) | Slack-style inbound via channel skills | **Yes** |
| **Send-on-behalf / draft external messages** | **Absent** | Email/Slack with confirmation | **Yes** |
| **Secure secret-request UI** | **Partial:** connector secrets in DB (`owner_connector_secrets`); Keychain on desktop; no masked in-chat secret widget | Masked input → env, not chat paste | Yes |
| **Streaming chat UX** | **Shipped:** SSE `GET /v1/runs/{id}/events`, `bot-conversation-view.tsx` | Streaming | Parity |
| **Durable background work** | **Shipped:** admission, queue, recovery, cancellation (`work_admission.rs`, `worker.rs`) | Background execution | **Elsewhere strength** (explicit Postgres model) |
| **Results / artifacts** | **Shipped:** `work_results`, immutable downloads (`results.rs`, `/api/results/[id]/download`) | Artifacts, file handoff | Partial — Grok emphasizes cross-machine handoff |
| **ChatGPT subscription execution** | **Shipped:** Codex device sign-in, `codex-provider`, no paid API fallback on auto-select (`PHASE_3C1.md`) | (Uses xAI / Cursor stack — different provider) | **Elsewhere differentiator** |
| **Auth providers** | **Shipped:** Better Auth email + optional Google (`apps/www/lib/auth.ts`) | (Not specified in brief) | N/A |
| **Billing (product)** | **Absent:** infra cost docs only; no Stripe/plans | (Not detailed in brief) | Unknown |
| **Safety / untrusted data** | **Partial:** redaction (`redact.rs`), group router fencing (`group_router.rs`), browser SSRF policy; no dedicated refusal module | Strong refusal policy; untrusted-data fencing | Partial gap on policy productization |
| **Bot export / share template** | **Absent** | Export bot template | Yes |
| **Box recovery settings** | **Partial:** runner drain, MCP cancel on drop; no user-facing “box debugging” product | Box debugging / recovery in settings | Yes |

**Legend:** *Shipped* = wired in code and product path; *Partial* = slice exists with documented limits; *Absent* = no meaningful implementation.

---

## Grok Bot advantages — what Elsewhere lacks (ranked)

Concrete gaps where the brief describes Grok Bot capabilities with no equivalent (or only a thin slice) in this repo.

### 1. Agent orchestration depth (highest product gap)

- **Background subagents** (`generalPurpose`, `computerUse`, `videoReview`) with todo queues, resume, redirect, and stop — **not present** (no subagent scheduler or types in `crates/` or `cloud-host`).
- **Cloud coding agents** for repository work (branch/PR across GitHub/GitLab/Bitbucket/Azure DevOps/Origin) without local clone — **not present**; GitHub integration is **read-only connector tools** (`connectors/github_client.rs`, `connector_tools.rs`), not an agentic PR workflow.
- **Deep Cursor integration** (IDE, cloud agents, plugin marketplace namespaces) — **not part of Elsewhere**; Elsewhere is a standalone control plane around Codex + Sprites.

### 2. Connectors, MCP, and extensibility

- **In-product MCP/plugin marketplace** with install-and-auth-in-place — Elsewhere has **one** first-party MCP server (`computer-mcp`) plus optional **GitHub OAuth** connector; no catalog of Convex/Prisma/Gmail/Neon-style plugins.
- **User-defined MCP servers** and **dynamic discovery** across many namespaces — only fixed tool exposure via Codex MCP config.
- **Outside event sources** for routines (Slack, GitHub, Origin, Teams, Linear, Sentry, PagerDuty, webhooks) — Elsewhere routines are **time-based only** (`schedule.rs`, `routines.rs`).

### 3. Collaboration and channels

- **SendToAgent**, **CreateAgent**, **UpdateAgent**, **CreateChannel** as first-class orchestration — Elsewhere has **`bot_delegate`** (async queue only) and **group conversations** without completing the loop (no auto-wake delegator, no multi-bot parallel Codex on one host per `PRODUCT_PROGRESS.md` §9).
- **Inbound Slack / external channels** and **send-on-behalf** messaging with confirmation discipline — **not implemented**.
- **Group chat “room turn” skill** as a productized playbook — groups exist in API/UI but not at Grok’s skill maturity.

### 4. Memory and continuity

- **Tiered memory** (profile / log / note) with agent-scoped and shared user-scoped facts — Elsewhere has a **single 16KB owner-edited `bot_context`** snapshotted at admission; **no automatic extraction** (`ROADMAP.md`, `BACKGROUND_WORK.md`).
- Grok’s durable “learn over time” model vs Elsewhere’s explicit “you write the memory” model is a major UX and capability gap.

### 5. Desktop-native product surface

- **Primary desktop chat** with multi-bot sidebar, hidden chats, and per-bot desktop/browser window on a **shared box filesystem** — Elsewhere’s primary surface is **`apps/www`**; Tauri is secondary; **no hidden chats**; computers are **per-bot Sprites**, not one shared box with multiple desktops.
- **Register local Mac** for approved shell/read/transfer alongside the cloud box — **LocalMacComputer** is a **desktop-only** adapter, not a hosted “target my laptop” feature.
- **CopyToBox / CopyFromBox** between user machines and the agent environment — **not implemented** as product APIs (workspace I/O only).

### 6. Chat UX affordances

- **In-chat forms** (`request_user_form`), **question widgets**, **request_box_help** for manual handoff — **not in** `bot-conversation-view.tsx` / message renderers.
- **Voice memos** (spoken replies) — mic **disabled** in desktop composer.
- **Rich attachments** in product chat — upload path **missing** on web; stub on desktop.

### 7. Skills and playbooks

- **Plugin-managed skills** and **learn-from-demonstration → reusable skill** — Elsewhere supports **versioned owner skills** (`agent-skills`, materialize into Codex run) but not Grok’s plugin skill ecosystem or demo capture.
- **Vertical domain playbooks** (calendar, food delivery, rideshare, flights, shopping, IRS refund, etc.) — **absent**.

### 8. Trust, secrets, and recovery productization

- **Auto-review** as a broad safety gate across Shell/MCP/computer/cloud actions — Elsewhere has **tool-level approvals** and Codex `approve` modes but not Grok’s unified auto-review cards across all risky classes.
- **Masked secret-request UI** routing secrets to env — connector secrets stored server-side; **no** in-chat secret widget pattern from the brief.
- **Box debugging / recovery** in user settings — operational drain/MCP teardown exists; **no** Grok-style owner recovery UX.

### 9. Bot lifecycle extras

- **Export / share bot template** — **absent**.
- **CreateAgent / UpdateAgent** from chat — **absent** (bots created via dashboard API only).

---

## What Elsewhere has (or aims for) that Grok Bot may not emphasize

Fair reverse comparison — strengths and deliberate choices in this repo, not claims about Grok’s private roadmap.

| Area | Elsewhere evidence |
|------|-------------------|
| **ChatGPT / Codex subscription as default engine** | Device sign-in, private profile volume, subscription-only auto engine (`codex-provider`, `PHASE_3C1.md`, `run_engine_select.rs`) — uses the user’s ChatGPT allowance rather than positioning as a separate API product. |
| **Separately assignable persistent computers** | Each bot binds a `computer_id`; Fly Sprite lifecycle (`api/computers.rs`, `computer_registry.rs`) — isolation vs Grok’s shared FS model. |
| **Postgres-durable work orchestration** | Idempotent admission, single-runner lock, interrupted-vs-queued semantics, SSE replay with cursors (`BACKGROUND_WORK.md`, 60+ cloud-host tests). |
| **Human approval + browser takeover** | `tool_approval_requests`, `computer_control_leases`, owner browser preview and input (`BROWSER.md`, `browser-human-control.tsx`). |
| **Fixed-interval + calendar routines with overlap rules** | Atomic schedule advance, coalescing, pause on failure (`routines.rs`, `PRODUCT_PROGRESS.md` §3). |
| **Immutable result library** | `work_results`, owner-scoped download BFF, bounded capture from `/workspace/results/{run-id}`. |
| **Portable `AgentComputer` abstraction** | Same tool loop for Sprite and local VM (`ARCHITECTURE.md`, `agent-core`). |
| **Hosted alpha operations** | Fly + Supabase runbook, drain on SIGTERM, RLS, invitation-gated signup (`HOSTED_ALPHA_ACCEPTANCE.md`, `SUPABASE_ALPHA.md`). |
| **One-way bot delegation (initial slice)** | `bot_delegations`, delegation cards in work UI (`PRODUCT_PROGRESS.md` §9). |
| **Owner-authored skills with materialization** | `025_product_skills.sql`, `materialize_agents_skills` in Codex run. |

---

## Suggested priority gaps (if closing vs Grok Bot)

Ordered by leverage for “persistent teammate with a computer” positioning, not by copying Grok blindly.

1. **Subagent / computerUse orchestration** — A dedicated GUI/browser executor (even if backed by existing `browser_*` tools) with queue and visibility would close the largest “watch it work like Grok” gap without abandoning AgentComputer safety.
2. **Memory beyond manual context** — Structured tiers or automatic summarization with provenance and caps; keeps parity with Grok’s “remembers you” expectation while respecting `ROADMAP.md` controls.
3. **Event-driven routines** — Start with **GitHub** or **webhook** triggers reusing the same admission path as cron routines (`worker.rs` / `routines::tick`).
4. **Collaboration completion** — Wake delegator on recipient completion; bounded artifact relay (extend `artifact_handoff.rs`); clarify group vs delegate UX.
5. **Connector expansion** — Second connector (e.g. Slack or Google Calendar) using existing `owner_connectors` pattern before a full marketplace.
6. **Chat UX primitives** — Attachments, approval-adjacent **question** UI, optional voice — improves trust and delegation without new infrastructure.
7. **Local machine bridge (optional)** — If differentiating from pure cloud box: approved read/exec on owner Mac from cloud runner (large security/design lift).
8. **Cloud coding agent** — Only if product strategy aligns with Cursor; otherwise explicitly **out of scope** vs Grok.

---

## Shipped vs planned (Elsewhere summary)

| Shipped (code + tests + web UI) | Planned / sketched only |
|--------------------------------|-------------------------|
| Bots, computers, Codex/Responses engines, durable runs, approvals, routines (time), bot context, results, workspace presence, browser tools + human control (Sprite), skills (owner), GitHub connector (read), bot delegation (one-way), group conversations (partial), Better Auth, hosted alpha stack | Multi-agent parallel execution, event routines, auto memory, MCP marketplace, cloud PR agents, Slack channels, voice, in-chat forms, local Mac from cloud, hidden chats, subagents, domain playbooks, bot template export |

For implementation truth, prefer **`docs/PRODUCT_PROGRESS.md`** over older **`docs/ROADMAP.md`** bullets that still say groups/handoffs are “deliberately later” while delegation and group APIs already exist.

---

## References (Elsewhere)

- Architecture: `docs/ARCHITECTURE.md`, `docs/CLOUD_ARCHITECTURE.md`, `docs/BACKGROUND_WORK.md`, `docs/BROWSER.md`
- Product state: `docs/PRODUCT_PROGRESS.md`, `docs/ROADMAP.md`, `docs/PRODUCT_SPEC.md`
- Control plane routes: `services/cloud-host/src/app.rs`
- Agent tools: `crates/agent-core/src/tool_catalog.rs`, `crates/computer-mcp/src/tools.rs`
- Migrations: `services/cloud-host/migrations/`
