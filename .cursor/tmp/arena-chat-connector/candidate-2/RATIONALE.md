# Rationale — candidate 2

## Problem

Bot chat is run-centric: each user message spawns a run whose mid-run activity is replayed from durable SSE events. GitHub access failures today surface as generic activity lines and the model asks for a repo URL, which cannot grant access. The product needs an in-thread connect card (owner-scoped GitHub App OAuth) and a cleaner transcript layout via AI Elements, without migrating to AI SDK `useChat` or putting tokens on the computer. Approvals and questions already prove the pattern: durable request events, timeline items, NeedsYou cards, and host-side wait/resume. Connector need is the same class of human interruption, but triggered by connector-class tool errors instead of policy.

## Usage (caller's view)

See [USAGE.md](./USAGE.md). Timeline renders `kind: "connector"` like approvals; replay uses the same `applyRunStreamEvent` reducer; `BotRunThread` wraps existing run summaries in AI Elements conversation/message chrome; OAuth starts through `startGithubConnectorOAuth` with a bot return URL shared with Connectors settings.

## Shape

**Base architecture (shape A):** When GitHub tool execution hits a connector-class error, cloud-host **does not** stream a failed `tool_result` to the UI as the primary signal. It opens a `ConnectorGate` (approval-shaped), persists and emits `connector_needed`, marks the run as waiting, and blocks the agent loop until `connector_resolved`. The www app parses wire payloads once into `ConnectorNeededPayload` / `ConnectorNeedPhase` and stores them on `RunActivityItem`. `ConnectorNeedCard` branches on `ConnectorNeedReason` (`not_connected`, `reconnect_required`, `repo_not_authorized`) for copy and CTA labels, then calls the existing OAuth start endpoint with `returnTo` pointing at `/app/bots/:botId` (and optional query for `runId`).

**AI Elements:** Install only `conversation`, `message`, and `prompt-input` as layout shells. Data flow remains run summaries + `RunActivityItem[]` + assistant stream state. No `useChat`, no UIMessage migration.

**Interface depth:** Callers (`RunConversationTimeline`, historical loader, `BotConversationView`) never inspect `errorCode` or tool error strings. All classification lives in `connector_gate/tool_intercept.rs` (write path) and `connector-need/from-stream.ts` (read path). `map-tool-error.ts` is a single lookup table shared conceptually with Rust—one invariant, one place.

**Invariants in types:** `repo_not_authorized` requires `repository`; host validates before emit. One pending need per run. Resolved events patch phase on the matching `connectorNeedId` idempotently.

**Deliberately not done:** Parsing `tool_result` SSE in the browser (shape B). Hiding GitHub tools from the catalog or preflight-only connect (shape C)—grounding requires tools to stay visible and failures to appear mid-run without failing the run.

## Synthesis decision

Filled by orchestrator.

## Tradeoffs accepted

- We accept a new durable event pair and host gate service in exchange for the same replay and mental model as approvals, without brittle UI-side classification of legacy `tool_result` errors.
- We accept a paused run (user-visible waiting) in exchange for preventing the model from improvising URLs/password prompts when GitHub is missing.
- We accept OAuth `state` / callback changes to honor `returnTo` in exchange for landing users back on the bot conversation instead of the connectors index.
- We accept a thin AI Elements dependency in exchange for consistent message/conversation spacing while keeping all state in existing run/timeline contexts.
- We accept host/agent coupling on gate resume in exchange for not streaming misleading success/failure tool lines during a connect flow.

## Alternatives considered

- **Shape B (classify existing `tool_result` SSE):** Smaller protocol change, but pushes connector detection to the www replay layer unless every historical failure was emitted with stable machine-readable fields anyway; today messages differ by code and prose. Loses on interface depth—timeline and replay would depend on tool error taxonomy leaking into React. Rejected as the primary shape.
- **Shape C (preflight / omit tools until connected):** Hides mid-run recovery and conflicts with “GitHub tools always in catalog” and non-terminal mid-run failures. Exposes connection state to the composer and empty-state paths instead of the run timeline. Rejected.
- **Terminal `RunFailureCard` for GitHub:** Matches ChatGPT-missing pattern but grounding marks GitHub as mid-run, not terminal. Rejected.

## Open questions and risks

- Should OAuth return auto-call `resolve_connected` on the pending need, or only refresh connection status and require an explicit “Continue” click on the card?
- When the user dismisses the card, does the run resume with a synthetic tool denial to the model, or stay paused until cancelled?
- For `repo_not_authorized`, is “Update GitHub access” always the GitHub App installation settings URL, or the same `oauth/start` flow?
- Do we suppress the activity line (“Listing GitHub repositories did not complete”) when gating, or show both line + card?
- Agent-core vs cloud-host: is tool intercept 100% in host today, or does agent-core need a small RPC to enter the gate?

## Next implementation step

Add `connector-need/types.ts` and extend `RunActivityItem` + `applyRunStreamEvent` with `connector_needed` / `connector_resolved` handlers (feature-flagged), then implement `ConnectorNeedCard` with `startGithubConnectorOAuth` and a stub payload in Vitest before touching Rust gate logic.
