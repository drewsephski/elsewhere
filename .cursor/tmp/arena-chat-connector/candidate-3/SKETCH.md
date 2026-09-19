# Candidate 3 — SKETCH

**Committed shape:** **A** — host emits durable `connector_needed` / `connector_resolved` SSE (parallel to `approval_requested`), run blocks on a connector wait registry until the owner connects or the need is superseded. UI renders a full-size `NeedsYouCard` GitHub connector card. OAuth `returnTo` restores `/app/bots/:botId` (same conversation).

**Not chosen:** B (classifying `tool_result` in the UI — leaks wire format, brittle replay), C (preflight-only — fights “tools always in catalog” and mid-run discovery).

---

## Usage-derived public surface (summary)

See `USAGE.md`. Types below are derived from those call sites.

---

## Domain types (single source of truth)

### Shared contract (`packages/run-events` or mirrored in agent-core + `apps/www/lib`)

```typescript
/** Provider id for connector needs today; extend without breaking consumers. */
export type ConnectorProviderId = "github";

/**
 * Why the run paused. Discriminant is the product copy + CTA, not tool error strings.
 * Parsed once at the SSE boundary; never re-derived in React.
 */
export type ConnectorNeedReason =
  | { kind: "not_connected"; provider: ConnectorProviderId }
  | { kind: "reconnect_required"; provider: ConnectorProviderId }
  | {
      kind: "repository_unauthorized";
      provider: ConnectorProviderId;
      owner: string;
      repo: string;
    }
  | { kind: "empty_installation"; provider: ConnectorProviderId };

export type ConnectorNeedOutcome = "connected" | "cancelled" | "superseded" | "expired";

/** Durable payload for connector_needed SSE + DB row. */
export interface ConnectorNeedPayload {
  connectorNeedId: string;
  runId: string;
  botId: string;
  botName?: string;
  /** Tool that triggered the need (for activity context, not for classification). */
  blockedTool: string;
  reason: ConnectorNeedReason;
  /** ISO timestamp from host. */
  requestedAt: string;
}

export interface ConnectorNeedResolvedPayload {
  connectorNeedId: string;
  runId: string;
  outcome: ConnectorNeedOutcome;
}
```

### Rust mirror (`crates/agent-core/src/connector_need.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorNeedReason { /* serde tagged enum matching TS */ }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorNeedPayload { /* fields match TS */ }

/// Map execution-time connector/github errors → structured reason (boundary only).
pub fn connector_need_reason_from_tool_error(
    tool_name: &str,
    tool_error: &ToolError,
    tool_args: &Value,
) -> Option<ConnectorNeedReason> {
    not_implemented!()
}
```

### Timeline union (`apps/www/contexts/active-run-context.tsx`)

```typescript
export type RunActivityItem =
  | { id: string; kind: "text"; text: string; technical?: string }
  | { id: string; kind: "approval"; approval: ApprovalRequestedPayload; decision?: ApprovalTerminalState }
  | { id: string; kind: "question"; question: UserQuestionPayload }
  | { id: string; kind: "subagent"; subagent: SubagentActivity }
  | {
      id: string;
      kind: "connector";
      need: ConnectorNeedPayload;
      outcome?: ConnectorNeedOutcome;
    };
```

### Card presentation model (derived, not stored)

```typescript
export interface ConnectorNeedPresentation {
  title: string;
  reason: string;
  detail?: string;
  primaryActionLabel: string;
  tone: NeedsYouTone;
}

export function presentationForConnectorNeed(
  need: ConnectorNeedPayload,
  outcome?: ConnectorNeedOutcome,
): ConnectorNeedPresentation {
  // TODO: switch on need.reason.kind + outcome
  not_implemented();
}
```

---

## Function signatures

### WWW — parse at SSE boundary

`apps/www/lib/connector-need.ts`

```typescript
export function connectorNeedFromPayload(
  runId: string,
  payload: Record<string, unknown>,
): ConnectorNeedPayload | null;

export function connectorNeedResolvedFromPayload(
  payload: Record<string, unknown>,
): ConnectorNeedResolvedPayload | null;

export function isGithubConnectorNeed(need: ConnectorNeedPayload): boolean;
```

`apps/www/lib/run-event-timeline.ts`

```typescript
// Inside applyRunStreamEvent:
// - "connector_needed" → append { kind: "connector", need } (dedupe by connectorNeedId)
// - "connector_resolved" → map matching item, set outcome (idempotent)
```

### WWW — OAuth (shared with Connectors settings)

`apps/www/lib/github-oauth.ts`

```typescript
export interface StartGithubOAuthOptions {
  /** Must be a validated in-app path, e.g. `/app/bots/${botId}?conversation=${id}` */
  returnTo: string;
}

/** Single implementation used by ConnectorsManager and in-chat card. */
export async function startGithubConnectorOAuth(
  options?: StartGithubOAuthOptions,
): Promise<void> {
  // POST /v1/connectors/github/oauth/start { returnTo }
  // window.location.href = authorizeUrl
  not_implemented();
}

export function botConversationReturnTo(botId: string, conversationId?: string): string {
  not_implemented();
}
```

`apps/www/app/app/connectors/github/callback/page.tsx`

```typescript
// After oauth/complete: router.replace(returnTo from completion response OR session fallback)
```

### WWW — UI

`apps/www/components/app/github-connector-card.tsx`

```typescript
interface GithubConnectorCardProps {
  need: ConnectorNeedPayload;
  outcome?: ConnectorNeedOutcome;
  botId: string;
  conversationId?: string;
}

export function GithubConnectorCard(props: GithubConnectorCardProps): JSX.Element {
  // NeedsYouCard full size, GitHubLogo, primary → startGithubConnectorOAuth({ returnTo })
  not_implemented();
}
```

`apps/www/components/app/workspace/run-conversation-timeline.tsx`

```typescript
// item.kind === "connector" && isGithubConnectorNeed → <GithubConnectorCard ... />
```

### WWW — AI Elements transcript shell (run-centric)

`apps/www/components/app/workspace/bot-run-transcript.tsx`

```typescript
interface BotRunTranscriptProps {
  runs: RunSummary[]; // existing shape from bot-conversation-view
  activeRunId: string | null;
  renderRun: (run: RunSummary) => ReactNode; // injects timeline + assistant bubble
}

/**
 * Replaces ad-hoc scroll div in bot-conversation-view.
 * Uses ai-elements Conversation + ConversationContent for stick-to-bottom.
 * Does NOT use useChat or UIMessage.
 */
export function BotRunTranscript(props: BotRunTranscriptProps): JSX.Element {
  not_implemented();
}
```

`apps/www/components/app/workspace/bot-run-message.tsx`

```typescript
interface BotRunMessageProps {
  role: "user" | "assistant";
  children: ReactNode;
}

/** Thin wrapper: ai-elements Message + MessageContent (+ MessageResponse for assistant markdown). */
export function BotRunMessage(props: BotRunMessageProps): JSX.Element {
  not_implemented();
}
```

`apps/www/components/app/run-activity-line.tsx` (or sibling)

```typescript
/** When activity line is tool-scoped, render ai-elements Tool instead of plain text. */
export function RunActivityToolLine(props: { headline: string; technical?: string }): JSX.Element {
  not_implemented();
}
```

`apps/www/components/app/workspace/bot-conversation-view.tsx`

```typescript
// Pseudocode:
// - Map each RunSummary block: BotRunMessage(user) → RunConversationTimeline → BotRunMessage(assistant)
// - On mount: if searchParams connectorResume=1, poll run events / refresh active run (idempotent)
```

### Cloud host — durable need + wait (mirrors approval)

`services/cloud-host/src/connector_need/service.rs`

```rust
pub struct ConnectorNeedService {
    pool: PgPool,
    registry: Arc<ConnectorWaitRegistry>,
    timeout: Duration,
}

impl ConnectorNeedService {
    /// Idempotent per (run_id, reason_kind, blocked_tool, repo key): returns existing pending id if match.
    pub async fn request_need(
        &self,
        run_id: &str,
        owner_id: &str,
        payload: ConnectorNeedPayload,
        events: &CloudEventSink,
    ) -> Result<String, ConnectorNeedError> {
        not_implemented!()
    }

    pub async fn wait_for_resolution(
        &self,
        connector_need_id: &str,
    ) -> Result<ConnectorNeedOutcome, ConnectorNeedError> {
        not_implemented!()
    }

    pub async fn resolve_on_github_connected(
        &self,
        owner_id: &str,
        run_id: Option<&str>,
    ) -> Result<u64, sqlx::Error> {
        // Called from oauth/complete: resolve all pending github needs for owner (and optional run)
        not_implemented!()
    }
}
```

`services/cloud-host/src/connector_need/registry.rs`

```rust
pub struct ConnectorWaitRegistry { /* oneshot map like ApprovalWaitRegistry */ }

pub async fn notify_resolved(&self, connector_need_id: &str, outcome: ConnectorNeedOutcome);
```

`services/cloud-host/src/api/connectors/github.rs` (or existing module)

```rust
/// POST /v1/connectors/github/oauth/start
/// Body: { returnTo?: string } — validated relative /app/* path, stored in oauth state row.
pub async fn github_oauth_start(...) -> ... {
    not_implemented!()
}

/// POST /v1/connectors/github/oauth/complete
/// Response adds { returnTo: string } for callback page redirect.
pub async fn github_oauth_complete(...) -> ... {
    // After token persist: connector_need_service.resolve_on_github_connected(...)
    not_implemented!()
}
```

### Agent core — block tool loop instead of feeding denial to model

`crates/agent-core/src/github_coding_tools.rs` (and `connector_tools.rs` for read tools)

```rust
/// When connector/github error is a "need owner action" class, call host hook instead of returning ToolError to the model.
pub trait ConnectorNeedGate: Send + Sync {
    async fn await_connector_need(
        &self,
        ctx: &ToolRunContext,
        reason: ConnectorNeedReason,
        blocked_tool: &str,
    ) -> Result<(), ToolError>;
}

// In tool handler:
// if let Some(reason) = connector_need_reason_from_tool_error(...) {
//     gate.await_connector_need(ctx, reason, GITHUB_OPEN_REPOSITORY_TOOL).await?;
//     // retry tool once after resolution
// }
```

Host wires `ConnectorNeedGate` to `ConnectorNeedService::request_need` + `wait_for_resolution`, then emits `connector_resolved` with `connected`.

---

## Module map

```
apps/www/
  lib/
    connector-need.ts          # domain types + SSE parsers + presentation helper
    github-oauth.ts            # startGithubConnectorOAuth, returnTo builders
    run-event-timeline.ts      # connector_needed / connector_resolved handlers
  components/
    app/
      github-connector-card.tsx
      workspace/
        bot-run-transcript.tsx       # AI Elements Conversation shell
        bot-run-message.tsx          # AI Elements Message wrappers
        run-conversation-timeline.tsx  # + connector branch
        bot-conversation-view.tsx      # compose transcript + composer (composer unchanged)
    ai-elements/
      conversation/            # pnpm dlx ai-elements add conversation
      message/                 # add message
      tool/                    # add tool (activity lines)
  app/app/connectors/github/callback/page.tsx  # honor returnTo

services/cloud-host/src/
  connector_need/
    mod.rs
    service.rs
    registry.rs
  approval/                    # unchanged; pattern reference only
  api/connectors/github.rs     # returnTo on start/complete

crates/agent-core/src/
  connector_need.rs            # reason enum + tool_error mapping
  github_coding_tools.rs       # gate integration
  connector_tools.rs           # gate for ConnectorError::NotConnected / ReconnectRequired

migrations/
  YYYYMMDD_connector_need_requests.sql   # durable rows (mirror tool_approval_requests)
```

---

## Data flow

```
Tool execute → connector_need_reason_from_tool_error (Some)
  → ConnectorNeedService.request_need → DB + SSE connector_needed
  → ConnectorWaitRegistry.wait
  → GithubConnectorCard → startGithubConnectorOAuth(returnTo=bot conversation)
  → GitHub App install → oauth/complete → resolve_on_github_connected
  → SSE connector_resolved { outcome: connected }
  → wait unblocks → tool retries → normal activity lines / assistant reply
```

Replay: `replayRunStreamEvents` replays the same `connector_needed` card with terminal `outcome` from `connector_resolved` — identical to approvals.

---

## AI Elements install scope

| Component      | Use |
|----------------|-----|
| `conversation` | Scroll + stick-to-bottom for multi-run transcript |
| `message`      | User + assistant bubbles per run |
| `tool`         | Tool activity lines (optional technical panel) |
| **Not** `prompt-input` | Keep `ChatComposerFrame` / attachments / preset prompts |
| **Not** `confirmation` | Keep `ApprovalCard` on `NeedsYouCard` |

---

## Invariants encoded in types

- `ConnectorNeedReason.kind` drives UX; tool `errorCode` never appears in components.
- `RunActivityItem.kind === "connector"` is the only timeline entry for connect cards.
- `returnTo` is always a host-validated path, never raw user input in the card.
- `connector_need_id` dedupes SSE + DB; resolution is idempotent.

---

## Deliberately not in this design

- Parsing `tool_result` in React for GitHub connect (shape B).
- Hiding GitHub tools from catalog (shape C).
- `useChat` / AI SDK message parts.
- Tokens on the computer or in the model context.
- `ask_user` for OAuth.
