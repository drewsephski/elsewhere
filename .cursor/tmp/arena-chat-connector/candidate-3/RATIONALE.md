# Rationale — candidate 3

## Problem

Bot chat is run-centric: each turn is a `RunSummary` with a user bubble, an activity timeline fed by durable SSE, and an assistant reply. GitHub tools stay in the catalog; missing access surfaces at execution as tool errors. Today those errors become opaque activity lines (“Listing GitHub repositories did not complete”) and the model falls back to asking for a repo URL, which cannot authorize access. The product needs an in-thread connect moment with correct copy for disconnected vs reconnect vs unauthorized repo, plus a transcript layout refresh using AI Elements without adopting `useChat`. The design must replay identically after refresh, share OAuth start with Connectors settings, and return the user to the same `/app/bots/:id` conversation after GitHub App install.

## Usage (caller's view)

See `USAGE.md`. The timeline renders connector cards like approvals; OAuth start is one shared function; the host owns blocking and resolution.

## Shape

**Protocol-first connector pause (shape A).** When a GitHub or connector read tool hits an owner-action error class, the host maps it once to `ConnectorNeedReason` (discriminated union), persists a row, emits `connector_needed`, and blocks the tool loop on `ConnectorWaitRegistry` until `connector_resolved`. The UI never inspects `tool_result` strings.

**Timeline domain model.** `RunActivityItem` gains `kind: "connector"` carrying `ConnectorNeedPayload` and optional `outcome`, parsed only in `connector-need.ts` and `applyRunStreamEvent`. Presentation copy is derived via `presentationForConnectorNeed`, keeping React free of error-code switches.

**OAuth return path.** `startGithubConnectorOAuth({ returnTo })` is extracted from `ConnectorsManager` into `lib/github-oauth.ts`. The host stores `returnTo` in OAuth state, returns it from `oauth/complete`, and the callback page `replace`s to the bot conversation (with optional `connectorResume` query for a one-shot active-run refresh). Settings Connectors passes `returnTo: "/app/connectors"` or omits it for the current default.

**Transcript layout.** `BotRunTranscript` wraps the existing per-run structure with AI Elements `Conversation` / `Message` / `Tool` only where they replace bespoke scroll and bubble markup. `ChatComposerFrame` and attachment flow stay. Custom interruption cards (`ApprovalCard`, `UserQuestionCard`, `GithubConnectorCard`) remain on `NeedsYouCard`; we do not install `prompt-input` or `confirmation`.

**Interface depth.** Callers (`RunConversationTimeline`, `bot-conversation-view`) only pass typed timeline items and bot ids. Complexity of SSE parsing, reason taxonomy, wait/resume, and OAuth validation sits behind `connector-need.ts`, `github-oauth.ts`, and `ConnectorNeedService`. The agent runtime sees a `ConnectorNeedGate` hook, not UI concerns.

Invariants in types: reason kind encodes UX; wire tool errors are boundary input only. Validation of `returnTo` is server-side. Resolution outcomes are idempotent on duplicate SSE.

## Synthesis decision

Filled by orchestrator.

## Tradeoffs accepted

- We accept a new durable event pair and DB table in exchange for replay-safe, typed connector cards without UI string matching on `tool_result`.
- We accept blocking the run at tool execution (like approvals) in exchange for stopping the model from improvising OAuth via chat text.
- We accept duplicating the approval wait pattern in `ConnectorNeedService` in exchange for a proven lifecycle and SSE semantics the frontend already understands.
- We accept a thin AI Elements dependency (conversation/message/tool) in exchange for layout consistency, while keeping the run-centric composer and card components we already ship.
- We accept mapping only “owner must connect” error classes to connector needs in exchange for leaving genuine validation failures as normal tool errors the model can correct.

## Alternatives considered

- **Shape B — classify `tool_result` in the UI:** Hides no complexity from the timeline layer; callers would need tool error conventions, historical replay would break if messages change, and unauthorized-repo vs not-connected would be re-inferred from strings. Rejected for shallow interface depth and violation of boundary-discipline.
- **Shape C — preflight before send:** Exposes connection state management to the composer and every entry path (presets, retries, delegations), and conflicts with “GitHub tools always in catalog.” Hides host discovery behind client heuristics. Rejected because mid-run recovery is the actual failure mode in the screenshot.
- **Terminal `RunFailureCard` for GitHub:** Matches Codex auth but GitHub gaps are recoverable without failing the run; rejected as the wrong lifecycle.

## Open questions and risks

- Should `repository_unauthorized` offer “open GitHub installation settings” in addition to reconnect, or only copy explaining App repo scope?
- When OAuth completes outside an active run (user connected from settings), should all pending needs for the owner resolve, or only needs tied to visible runs?
- Does bot presence (`waiting_approval`) need a sibling `waiting_connector` for sidebar badges, or is timeline-only sufficient for v1?
- AI Elements `Message` styling vs existing `UserPromptBubble` / `AssistantMessageBubble` — how much visual change is acceptable in one pass?
- Tool retry after resolution: single automatic retry vs resume entire agent turn — what happens if the second attempt fails for a different reason?

## Next implementation step

Add `connector_need_requests` migration and `ConnectorNeedService::request_need` emitting `connector_needed`, plus `connector_need_from_payload` and timeline handlers, before any AI Elements install.
