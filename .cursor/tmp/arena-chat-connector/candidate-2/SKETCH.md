# Sketch — candidate 2 (shape A: durable `connector_needed`)

Whole shape: **host emits first-class durable connector events; run pauses at the gate; UI renders a NeedsYou-style card; OAuth returns to the bot conversation.** AI Elements refresh is a parallel presentation layer on the existing run-centric model.

---

## Module map

| Module | Responsibility |
|--------|----------------|
| `apps/www/lib/connector-need/types.ts` | Domain discriminated unions (`ConnectorId`, `ConnectorNeedReason`, `ConnectorNeedPhase`, payloads). No SSE strings in UI. |
| `apps/www/lib/connector-need/map-tool-error.ts` | **Host/agent only** — maps Rust `ToolError` codes → `ConnectorNeedReason` (single table). Not imported by React. |
| `apps/www/lib/connector-need/from-stream.ts` | `connectorNeedFromStreamPayload`, `applyConnectorResolved` — parse/validate at SSE boundary. |
| `apps/www/lib/connectors/github-oauth.ts` | `startGithubConnectorOAuth`, `buildBotConversationReturnUrl` — shared by settings + chat card. |
| `apps/www/lib/run-event-timeline.ts` | Extend `applyRunStreamEvent` for `connector_needed` / `connector_resolved`. |
| `apps/www/contexts/active-run-context.tsx` | Extend `RunActivityItem` with `kind: "connector"`. |
| `apps/www/components/app/connector-need-card.tsx` | NeedsYou shell + reason-specific copy + Connect CTA. |
| `apps/www/components/app/bot-chat/bot-run-thread.tsx` | AI Elements `Conversation` layout over `RunSummary[]`. |
| `apps/www/components/app/bot-chat/bot-run-turn.tsx` | One user `Message` + timeline + assistant `Message`. |
| `apps/www/components/app/bot-chat/bot-chat-composer.tsx` | Thin `PromptInput` wrapper around existing composer hooks. |
| `services/cloud-host/src/connector_gate/` | Mirror `approval/`: persist event, emit SSE, wait, resolve, resume run. |
| `services/cloud-host/src/connectors/github/oauth.rs` | Accept `return_to` in OAuth `state`; callback redirect. |
| `services/agent-core/.../tool_dispatch.rs` (or host shim) | On connector-class tool failure, call gate instead of surfacing raw `tool_result` error to the model loop. |

---

## Domain types (canonical)

```ts
// apps/www/lib/connector-need/types.ts

export type ConnectorId = "github";

/** Why the gate fired — mutually exclusive UX branches. */
export type ConnectorNeedReason =
  | "not_connected"
  | "reconnect_required"
  | "repo_not_authorized";

/** Lifecycle of one need instance (encoded on timeline item + resolved event). */
export type ConnectorNeedPhase =
  | "pending"
  | "connected"
  | "dismissed"
  | "expired";

export interface ConnectorRepositoryRef {
  owner: string;
  repo: string;
}

export interface ConnectorNeededPayload {
  connectorNeedId: string;
  connector: ConnectorId;
  reason: ConnectorNeedReason;
  runId: string;
  botId: string;
  blockedTool: string;
  /** Stable id for idempotent resume (host-assigned). */
  blockedInvocationId: string;
  message: string;
  repository?: ConnectorRepositoryRef;
  waitingForConnection?: boolean;
}

export interface ConnectorResolvedPayload {
  connectorNeedId: string;
  phase: Exclude<ConnectorNeedPhase, "pending">;
  connector: ConnectorId;
}

export type ConnectorNeedTimelineItem = {
  id: string;
  kind: "connector";
  connector: ConnectorNeededPayload;
  phase: ConnectorNeedPhase;
};
```

```ts
// apps/www/contexts/active-run-context.tsx (extend union)

export type RunActivityItem =
  | { id: string; kind: "text"; text: string; technical?: string }
  | { id: string; kind: "approval"; approval: ApprovalRequestedPayload; decision?: ApprovalTerminalState }
  | { id: string; kind: "connector"; connector: ConnectorNeededPayload; phase: ConnectorNeedPhase }
  | { id: string; kind: "subagent"; subagent: SubagentActivity }
  | { id: string; kind: "question"; question: UserQuestionPayload };
```

---

## State machine (one `connectorNeedId`)

```text
                    connector_needed (durable)
                              │
                              ▼
                         ┌─────────┐
           OAuth success │ pending │
         ───────────────►│         │◄──────── dismiss / skip
                         └────┬────┘
                              │
         connector_resolved     │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
         connected        dismissed        expired
              │               │               │
              └───────────────┴───────────────┘
                         run resumes or
                         stays paused (policy)
```

**Invariants (types + host):**

- At most one `pending` connector need per `runId` (host rejects duplicate gate).
- `repo_not_authorized` ⇒ `repository` required.
- `not_connected` / `reconnect_required` ⇒ `repository` optional.
- Resume is idempotent: duplicate `connector_resolved` with same `connectorNeedId` is ignored.

---

## Function signatures

### WWW — boundary parsing

```ts
// apps/www/lib/connector-need/from-stream.ts

export function connectorNeedFromStreamPayload(
  payload: Record<string, unknown>,
): ConnectorNeededPayload | null;

export function connectorResolvedFromStreamPayload(
  payload: Record<string, unknown>,
): ConnectorResolvedPayload | null;

export function applyConnectorResolvedToItems(
  items: RunActivityItem[],
  resolved: ConnectorResolvedPayload,
): RunActivityItem[];
```

```ts
// apps/www/lib/run-event-timeline.ts

export function applyRunStreamEvent(
  state: RunTimelineState,
  runId: string,
  streamEvent: RunStreamEvent,
  options?: { skipIfSeenId?: (id: string) => boolean },
): RunTimelineState;
// + branches:
//   streamEvent.event === "connector_needed"
//   streamEvent.event === "connector_resolved"
```

### WWW — OAuth (shared)

```ts
// apps/www/lib/connectors/github-oauth.ts

export interface GithubOAuthStartOptions {
  returnTo: string;
}

export function buildBotConversationReturnUrl(args: {
  botId: string;
  conversationId?: string;
  runId?: string;
  connector?: ConnectorId;
}): string;

export async function startGithubConnectorOAuth(
  options: GithubOAuthStartOptions,
): Promise<void>;
// POST /v1/connectors/github/oauth/start with { returnTo }
// window.location.assign(authorizationUrl)
```

### WWW — UI

```tsx
// apps/www/components/app/connector-need-card.tsx

export interface ConnectorNeedCardProps {
  payload: ConnectorNeededPayload;
  externalPhase?: ConnectorNeedPhase;
  botId: string;
  onPhaseChange?: (phase: ConnectorNeedPhase) => void;
}

export function ConnectorNeedCard(props: ConnectorNeedCardProps): JSX.Element;
```

```tsx
// apps/www/components/app/bot-chat/bot-run-thread.tsx

export interface BotRunThreadProps {
  runs: RunSummary[];
  liveRunId: string | null;
  getTimeline: (runId: string) => RunActivityItem[];
  getAssistantBody: (run: RunSummary) => React.ReactNode;
  composer: React.ReactNode;
  headerSlot?: React.ReactNode;
}

export function BotRunThread(props: BotRunThreadProps): JSX.Element;
```

### Cloud-host — gate service (Rust sketch)

```rust
// services/cloud-host/src/connector_gate/service.rs

pub struct ConnectorGate { /* pool, events bus */ }

pub struct ConnectorGateContext {
    pub run_id: String,
    pub bot_id: String,
    pub owner_id: String,
    pub connector: ConnectorId,
    pub reason: ConnectorNeedReason,
    pub blocked_tool: String,
    pub blocked_invocation_id: String,
    pub repository: Option<RepositoryRef>,
    pub user_message: String,
}

impl ConnectorGate {
    /// Persist + SSE `connector_needed`, block until resolved or cancel.
    pub async fn wait_for_connection(
        &self,
        ctx: ConnectorGateContext,
        cancel: CancellationToken,
    ) -> Result<ConnectorResolution, ConnectorGateError>;

    /// Called from OAuth callback or explicit user action.
    pub async fn resolve_connected(
        &self,
        owner_id: &str,
        connector_need_id: &str,
    ) -> Result<(), ConnectorGateError>;

    pub async fn resolve_dismissed(
        &self,
        owner_id: &str,
        connector_need_id: &str,
    ) -> Result<(), ConnectorGateError>;
}

pub enum ConnectorResolution {
    Connected,
    Dismissed,
    Expired,
    Cancelled,
}
```

```rust
// services/cloud-host/src/connector_gate/tool_intercept.rs

/// Called when a connector tool returns a connector-class error BEFORE emitting tool_result to SSE.
pub fn classify_github_tool_error(err: &ToolError) -> Option<ConnectorNeedReason>;

pub async fn maybe_gate_instead_of_tool_result(
    gate: &ConnectorGate,
    ctx: ToolExecutionContext,
    err: ToolError,
) -> GateDecision;

pub enum GateDecision {
    EmitToolResultAsToday,
    Gated { need_id: String },
}
```

### Agent-core — cooperation

```rust
// Pseudocode — host-owned wait; agent loop receives "retry tool" signal after gate.

pub async fn execute_tool_with_host_gate(...) -> ToolOutcome {
    match host.execute_tool(...) {
        Ok(v) => ToolOutcome::Success(v),
        Err(e) if e.is_connector_gate() => ToolOutcome::WaitingConnector(e.need_id()),
        Err(e) => ToolOutcome::Failed(e),
    }
}
```

---

## `applyRunStreamEvent` pseudocode (connector branches)

```ts
if (streamEvent.event === "connector_needed") {
  const connector = connectorNeedFromStreamPayload(payload);
  if (!connector) return state;
  if (items.some((i) => i.id === id)) return state;
  // Collapse duplicate pending for same connectorNeedId
  return {
    items: [...items, { id, kind: "connector", connector, phase: "pending" }],
    pendingHumanIntervention,
  };
}

if (streamEvent.event === "connector_resolved") {
  const resolved = connectorResolvedFromStreamPayload(payload);
  if (!resolved) return state;
  return {
    items: applyConnectorResolvedToItems(items, resolved),
    pendingHumanIntervention,
  };
}
```

---

## `ConnectorNeedCard` pseudocode

```tsx
export function ConnectorNeedCard({ payload, externalPhase, botId, onPhaseChange }: ConnectorNeedCardProps) {
  const [phase, setPhase] = useState<ConnectorNeedPhase>(externalPhase ?? "pending");
  const needsYouVariant = phase === "pending" ? "pending" : phase === "connected" ? "resolved" : "neutral";

  const title = copyForReason(payload.reason); // not_connected | reconnect | repo
  const primaryCta =
    payload.reason === "repo_not_authorized"
      ? "Update GitHub access"
      : payload.reason === "reconnect_required"
        ? "Reconnect GitHub"
        : "Connect GitHub";

  async function handleConnect() {
    await startGithubConnectorOAuth({
      returnTo: buildBotConversationReturnUrl({
        botId,
        runId: payload.runId,
        connector: payload.connector,
      }),
    });
  }

  return (
    <NeedsYouCard variant={needsYouVariant} title={title} description={payload.message}>
      {phase === "pending" ? (
        <>
          <Button onClick={handleConnect}>{primaryCta}</Button>
          <Button variant="ghost" onClick={() => dismissNeed(payload.connectorNeedId)}>Not now</Button>
        </>
      ) : null}
      {payload.repository ? (
        <p className="text-muted-foreground text-sm">{payload.repository.owner}/{payload.repository.repo}</p>
      ) : null}
    </NeedsYouCard>
  );
}
```

---

## AI Elements layout (minimal install)

| AI Element | Replaces |
|------------|----------|
| `conversation` | Ad-hoc scroll container + spacing in `bot-conversation-view` |
| `message` | Raw div wrappers around `UserPromptBubble` / assistant markdown block |
| `prompt-input` | Outer chrome only; inner behavior stays `ChatComposerTextarea` + attachments |

**Not installed:** `tool`, `confirmation` (approvals/connector keep `NeedsYouCard` + existing cards). `shimmer`/`sources`/`suggestion` stay as today.

```tsx
// bot-run-turn.tsx — structural pseudocode
<Conversation>
  {runs.map((run) => (
    <Fragment key={run.id}>
      <Message from="user">
        <UserPromptBubble ... />
      </Message>
      <Message from="assistant">
        <RunConversationTimeline items={getTimeline(run.id)} botId={botId} />
        <RunAssistantSnippet ... />
      </Message>
    </Fragment>
  ))}
  {composer}
</Conversation>
```

---

## SSE event payloads (wire → domain)

```json
// connector_needed
{
  "connectorNeedId": "cn_…",
  "connector": "github",
  "reason": "not_connected",
  "runId": "run_…",
  "botId": "bot_…",
  "blockedTool": "github_list_repositories",
  "blockedInvocationId": "inv_…",
  "message": "Connect GitHub so DrewOS can list your repositories.",
  "waitingForConnection": true
}

// connector_resolved
{
  "connectorNeedId": "cn_…",
  "connector": "github",
  "phase": "connected"
}
```

---

## Tests (sketch)

| Test | Location |
|------|----------|
| `replayRunStreamEvents` emits connector item + phase update | `run-event-timeline.test.ts` |
| `connectorNeedFromStreamPayload` rejects invalid reason/repo combo | `connector-need/from-stream.test.ts` |
| `ConnectorNeedCard` copy per reason | `connector-need-card.test.tsx` |
| Gate idempotency + single pending per run | `services/cloud-host/tests/connector_gate.rs` |
