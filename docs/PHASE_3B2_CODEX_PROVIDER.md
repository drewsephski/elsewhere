# Phase 3B.2 — Codex subscription provider

**Goal:** Use **ChatGPT plan / Codex allowance** (via Codex + ChatGPT login) as Elsewhere’s **preferred** model path, while keeping **OpenAI API key** (`ResponsesRunEngine`) as pay-as-you-go fallback.

Phase 3B.1 (Postgres run store, cloud-host SSE, SpriteComputer) stays as-is. This phase replaces **model transport**, not computer or persistence work.

## Billing reality

| Auth path | ChatGPT subscription? | Separate API bill? |
|-----------|----------------------:|-------------------:|
| OpenAI API key | No | **Yes** |
| Codex + ChatGPT login | **Yes** | No API bill |
| ChatGPT/Codex credits (after allowance) | **Yes** | Not API credits |
| “Sign in with ChatGPT” (identity OAuth only) | N/A | N/A |

ChatGPT and the OpenAI API are billed separately. Codex signed in with ChatGPT uses plan usage; your own API key uses API pricing.

References: [Managing billing](https://help.openai.com/en/articles/9039756-managing-billing-settings-on-the-chatgpt-web-and-api-platform), [Using Codex with your ChatGPT plan](https://help.openai.com/en/articles/11369540).

## What we are *not* doing

- Reading `~/.codex/auth.json` or calling ChatGPT backends with extracted tokens.
- Treating generic “Sign in with ChatGPT” as subscription access (identity only).
- Forcing Codex to impersonate `ResponsesModel` for the full agent loop.

## Target architecture

```text
RunEngine
├── ResponsesRunEngine          (today)
│    ├── OpenAI API key
│    └── agent-core::run_agent_loop + ResponsesModel
│
└── CodexRunEngine                (3B.2)
     ├── ChatGPT subscription (Codex OAuth owned by Codex)
     └── Codex app-server (threads, tools, compaction)
```

Elsewhere exposes the computer to Codex as **MCP tools** (preferred) or equivalent app-server tool registration:

```text
Codex / Luna
     ↓
Elsewhere Computer MCP
     ├── workspace_list
     ├── workspace_read
     ├── workspace_write
     └── workspace_exec
            ↓
       AgentComputer
       ├── SpriteComputer   (cloud-host)
       └── LocalMacComputer (desktop)
```

**Codex owns:** model calls, reasoning, context, tool invocation, OAuth/refresh, session/compaction.

**Elsewhere owns:** persistent computer, cloud/local placement, bots, Postgres/SQLite, run history, routines, approvals, UI.

Default model when Codex + ChatGPT is available: `gpt-5.6-luna` (`agent_core::DEFAULT_MODEL`).

## Desktop startup flow

```text
Elsewhere
   ↓
Detect Codex (codex-provider)
   ↓
ChatGPT authenticated?
   ├─ yes → prefer "ChatGPT subscription" provider
   └─ no  → offer "Sign in with ChatGPT" (Codex login flow)
```

**Interim detection (3B.2a):** `which codex`, `codex --version`, `codex login status` (string parse only for bootstrap).

**Target detection (3B.2b):** Codex **app-server** account/auth APIs (`codex app-server` daemon), not CLI output. Generate protocol bindings via `codex app-server generate-json-schema` when wiring the client.

Do **not** depend on parsing `auth.json`.

## Settings UX (target)

```text
Model provider

● ChatGPT subscription
  Connected via Codex
  Account: user@…
  Model: Luna

○ OpenAI API
  Pay-as-you-go API key

○ Other provider
```

## Cloud path (investigate in 3B.2+)

```text
Browser → Elsewhere "Connect ChatGPT"
       → Codex login_chatgpt() / device or browser auth
       → per-user Codex profile on cloud-host
       → app-server → Luna → SpriteComputer
```

Treat multi-tenant credential storage and ToS as product/legal constraints; architecture should allow per-user Codex profiles without sharing one API key.

## Implementation checklist

1. [x] Document phase (this file) + `RunEngine` types in `agent-core`.
2. [x] `codex-provider` crate: install probe + auth state (CLI interim).
3. [x] `CodexRunEngine`: app-server client, thread lifecycle, event mapping → `EventSink` / `RunStore`.
4. [ ] Browser ChatGPT login via Codex-supported flow (`login_chatgpt` / `codex login`).
5. [ ] Elsewhere Computer MCP server (workspace_* → `AgentComputer`).
6. [ ] Desktop: prefer Codex when ChatGPT-authenticated; API key fallback.
7. [ ] Cloud-host: optional Codex profile per user (no `OPENAI_API_KEY` on subscription path).
8. [ ] Proof: ChatGPT subscription → Luna → Codex → Elsewhere tool → Fly Sprite.
9. [ ] Verify subscription path has **zero** `OPENAI_API_KEY`.
10. [ ] Verify usage counts against Codex/ChatGPT allowance, not API billing.

## Acceptance test (manual)

1. Remove or unset `OPENAI_API_KEY` for the run path under test.
2. `codex login status` → logged in with ChatGPT.
3. Start Elsewhere agent run with computer enabled (Sprite or local).
4. Confirm tool call hits Sprite/local computer and run completes.
5. Confirm no API-key Responses calls in logs/traces for that run.

## Deferred (Phase 3C+)

User/dashboard polish, routines, and approvals UI can follow once the subscription provider path is proven.

**Approvals (3C):** Phase 3B.2 uses Codex `approvalPolicy=never` with Elsewhere MCP `default_tools_approval_mode=approve` so Luna can call workspace tools without blocking the E2E gate. Product-facing approval for destructive computer actions (`workspace_write`, `workspace_exec`, etc.) must be enforced by **Elsewhere before `AgentComputer` dispatch**, not by relying on Codex approval UI alone.
