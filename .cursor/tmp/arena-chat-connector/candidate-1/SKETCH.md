# Type sketch

Committed shape. Host emits a durable `connector_needed` interruption, the agent tool call blocks, the UI renders a `NeedsYouCard`, OAuth returns to the same bot conversation. GitHub tools stay in the catalog. Access is classified into a typed `ConnectorNeedReason` before dispatch. Chat layout stays run-centric and uses AI Elements `conversation`, `message`, and `tool` as presentational replacements for existing custom chrome. It does not use `confirmation` or `prompt-input`. It does not adopt `useChat`.

## Domain types

```ts
/** Owner-scoped GitHub App. The only provider this design implements. */
export type ConnectorProvider = "github";

/**
 * Why the tool cannot run. Built by `classify_github_access`, never by matching
 * `ToolError` message strings.
 */
export type ConnectorNeedReason =
  | { kind: "disconnected" }
  | { kind: "reconnect_required" }
  | { kind: "unauthorized_repo"; owner: string; repo: string }
  | { kind: "empty_authorization" };

export type ConnectorNeedResolutionKind =
  | "connected"
  | "repo_authorized"
  | "dismissed"
  | "expired"
  | "cancelled";

export type ConnectorNeedStatus =
  | { phase: "pending" }
  | { phase: "resolved"; resolution: ConnectorNeedResolutionKind };

export type ConnectorNeed = {
  needId: string;
  provider: ConnectorProvider;
  reason: ConnectorNeedReason;
  runId: string;
  botId: string;
  toolName: string;
  requestedAt: string;
  status: ConnectorNeedStatus;
};

/**
 * Same-origin path the OAuth callback may navigate to.
 * Only `/app/bots/:botId` with optional `?conversation=:conversationId`.
 */
export type BotChatReturnTo = string & { readonly __brand: "BotChatReturnTo" };

export type GithubOAuthStartRequest = {
  returnTo?: BotChatReturnTo;
};

export type GithubOAuthStartResponse = {
  authorizeUrl: string;
  state: string;
  expiresAt: string;
};

export type GithubOAuthCompleteResponse = {
  provider: "github";
  status: string;
  metadata: Record<string, unknown>;
  connectedAt: string | null;
  updatedAt: string;
  returnTo: string | null;
};
```

Timeline item added next to `approval`. Tool activity is an upserted item, not a new headline per event.

```ts
export type ToolActivityState = "running" | "complete" | "error";

export type ToolActivity = {
  toolName: string;
  label: string;
  state: ToolActivityState;
  technical?: string;
};

export type RunActivityItem =
  | { id: string; kind: "text"; text: string; technical?: string }
  | { id: string; kind: "tool"; tool: ToolActivity }
  | { id: string; kind: "approval"; approval: ApprovalRequestedPayload; decision?: ApprovalTerminalState }
  | { id: string; kind: "connector"; need: ConnectorNeed }
  | { id: string; kind: "subagent"; subagent: SubagentActivity }
  | { id: string; kind: "question"; question: UserQuestionPayload };
```

Rust mirror in `crates/agent-core` stays error-shaped. Classification and waiting live in the host, because the install catalog and OAuth waiter already live there.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectorNeedReason {
    Disconnected,
    ReconnectRequired,
    UnauthorizedRepo { owner: String, repo: String },
    EmptyAuthorization,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GithubAccess {
    Ready,
    Need(ConnectorNeedReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectorNeedResolution {
    Connected,
    RepoAuthorized,
    Dismissed,
    Expired,
    Cancelled,
}

pub struct ConnectorNeedRequest {
    pub owner_id: String,
    pub run_id: String,
    pub bot_id: String,
    pub tool_name: String,
    pub reason: ConnectorNeedReason,
}
```

Illegal states the types refuse:

- A resolved need with no `resolution`.
- `unauthorized_repo` without `owner` and `repo`.
- A `returnTo` that is not a bot-chat path. `parseBotChatReturnTo` is the only constructor.
- UI status `connecting`. That is local button state, not durable.

## SSE contract

Event names match the approval pair.

`connector_needed`

```json
{
  "needId": "cneed_...",
  "provider": "github",
  "reason": { "kind": "disconnected" },
  "runId": "run_...",
  "botId": "bot_...",
  "toolName": "github_list_repositories",
  "requestedAt": "2026-09-19T17:00:00.000Z"
}
```

`reason` for a missing repo:

```json
{ "kind": "unauthorized_repo", "owner": "acme", "repo": "elsewhere" }
```

`connector_needed_resolved`

```json
{
  "needId": "cneed_...",
  "resolution": "connected"
}
```

Do not persist `tool_result` `{ "ok": false }` for a classified access miss. The need event is the durable record. Dismiss and timeout still return `ToolError` to the model after `connector_needed_resolved` with `dismissed` or `expired`.

Idempotency:

- A second `request_and_wait` for the same `run_id` + `provider` while `pending` joins the existing waiter and does not append a second event.
- `github_oauth_complete` notifies all pending GitHub needs for that owner. Each waiter reclassifies. Satisfied waiters resolve. Unsatisfied waiters stay pending on the same `needId`.
- Replaying `connector_needed` then `connector_needed_resolved` yields the resolved card. Duplicate event ids are skipped the same way approvals are.

## Signatures

### `apps/www/lib/connector-need.ts`

```ts
export function parseConnectorNeed(
  runId: string,
  payload: Record<string, unknown>,
): ConnectorNeed | null {
  throw new Error("not implemented");
}

export function parseConnectorNeedStatus(
  payload: Record<string, unknown>,
): ConnectorNeedStatus {
  throw new Error("not implemented");
}

export function botChatReturnTo(input: {
  botId: string;
  conversationId?: string | null;
}): BotChatReturnTo {
  throw new Error("not implemented");
}

export function parseBotChatReturnTo(raw: string): BotChatReturnTo | null {
  // TODO: allow only `/app/bots/:botId` and optional `conversation` query.
  // Reject protocol-relative URLs, other `/app/*` paths, and absolute foreign origins.
  throw new Error("not implemented");
}

export function connectorNeedCopy(need: ConnectorNeed): {
  title: string;
  reason: string;
  actionLabel: string;
  continuation: string;
} {
  throw new Error("not implemented");
}
```

Copy table (single source, used by the card):

| `reason.kind` | title | reason | actionLabel |
| --- | --- | --- | --- |
| `disconnected` | Connect GitHub? | This bot needs GitHub to work on repositories. | Connect GitHub |
| `reconnect_required` | Reconnect GitHub? | GitHub is linked but this account must sign in again. | Reconnect GitHub |
| `unauthorized_repo` | Add this repository? | GitHub is connected, but `{owner}/{repo}` is not in the authorized installation. | Add {owner} on GitHub |
| `empty_authorization` | Grant repository access? | GitHub is connected, but no repositories are authorized yet. | Add repositories |

Pending continuation is always `Your bot continues after you connect.` Resolved cards use `NeedsYouCard` tone `resolved`. They do not keep a Connect button.

### `apps/www/lib/github-oauth.ts`

```ts
export async function startGithubConnectorOAuth(options?: {
  returnTo?: BotChatReturnTo;
}): Promise<GithubOAuthStartResponse> {
  throw new Error("not implemented");
  // POST /v1/connectors/github/oauth/start
  // body: options?.returnTo ? { returnTo: options.returnTo } : {}
}
```

`ConnectorsManager.handleConnectGithub` calls this with no `returnTo`. `ConnectorNeedCard` always passes `returnTo`.

### `apps/www/lib/run-event-timeline.ts`

Extend `applyRunStreamEvent` with the two connector events. Upsert `kind: "tool"` from events that already carry a tool name, keyed by `payload.toolInvocationId` when present, otherwise by `toolName` plus the current open tool slot.

```ts
function upsertToolItem(
  items: RunActivityItem[],
  id: string,
  tool: ToolActivity,
): RunActivityItem[] {
  throw new Error("not implemented");
}
```

Skip the existing `tool_result && ok === false` headline when `payload.errorCode` is a connector-access miss. New runs will not emit that payload. Keep the headline for older stored events so replay of past runs does not invent connector cards from strings.

### `apps/www/components/app/connector-need-card.tsx`

```tsx
export function ConnectorNeedCard(props: {
  need: ConnectorNeed;
  returnTo: BotChatReturnTo;
}): JSX.Element {
  throw new Error("not implemented");
  // NeedsYouCard, tone pending | resolved
  // compact={false} (same as questions, not approvals)
  // actions: Connect / Reconnect / Add repositories
  // local busy flag only; do not patch need.status to a fake "connecting"
}
```

### Host

```rust
// services/cloud-host/src/connectors/github_access.rs
pub async fn classify_github_access(
    pool: &PgPool,
    secret_box: &ConnectorSecretBox,
    owner_id: &str,
    tool_name: &str,
    arguments: &Value,
) -> Result<GithubAccess, ApiError> {
    unimplemented!("load credential; map Missing/Legacy/ReconnectRequired; for repo tools check catalog; for list/search treat empty catalog as EmptyAuthorization");
}

// services/cloud-host/src/connector_need/service.rs
impl ConnectorNeedService {
    pub async fn request_and_wait(
        &self,
        request: ConnectorNeedRequest,
        cancel: &AtomicBool,
    ) -> Result<ConnectorNeedResolution, ConnectorNeedError> {
        unimplemented!("insert pending row if none for run+provider; append+emit connector_needed; wait; on notify reclassify; emit connector_needed_resolved when satisfied");
    }

    pub async fn notify_owner_github_changed(&self, owner_id: &str) -> Result<(), ApiError> {
        unimplemented!("wake pending GitHub waiters for this owner after oauth complete or disconnect");
    }

    pub async fn dismiss(
        &self,
        owner_id: &str,
        need_id: &str,
    ) -> Result<(), ApiError> {
        unimplemented!("owner-auth'd optional dismiss; emit resolved dismissed");
    }
}
```

OAuth persistence:

```rust
pub async fn store_oauth_state(
    pool: &PgPool,
    state: &str,
    owner_id: &str,
    provider: &str,
    expires_at: DateTime<Utc>,
    return_to: Option<&str>,
) -> Result<(), ApiError> {
    unimplemented!("INSERT includes return_to; existing rows without it stay null")
}

pub async fn consume_oauth_state(
    pool: &PgPool,
    state: &str,
    provider: &str,
    owner_id: &str,
) -> Result<Option<ConsumedOAuthState>, ApiError> {
    unimplemented!("DELETE ... RETURNING return_to; None means invalid/expired")
}
```

`github_oauth_start` accepts optional JSON `{ "returnTo": string }`. Validate with the same rules as `parseBotChatReturnTo` before insert. `github_oauth_complete` returns `returnTo` from the consumed row. After upsert it calls `notify_owner_github_changed`.

Migration:

```sql
ALTER TABLE connector_oauth_states
  ADD COLUMN return_to text;

CREATE TABLE connector_need_requests (
  id uuid PRIMARY KEY,
  owner_id text NOT NULL,
  run_id text NOT NULL,
  bot_id text NOT NULL,
  provider text NOT NULL,
  tool_name text NOT NULL,
  reason_kind text NOT NULL,
  repo_owner text,
  repo_name text,
  status text NOT NULL,
  resolution text,
  created_at timestamptz NOT NULL,
  resolved_at timestamptz,
  UNIQUE (run_id, provider)
);
-- CHECK: unauthorized_repo requires repo_owner and repo_name
-- CHECK: other kinds require both repo columns null
```

One pending row per run and provider. That is the join key for idempotent waits.

### Callback and conversation restore

`apps/www/app/app/connectors/github/callback/page.tsx` uses `body.returnTo ?? "/app/connectors"`.

`BotConversationView` on load:

1. If `searchParams.get("conversation")` is a non-empty id, use it.
2. Else call today's `GET /v1/conversations?limit=1&bot_id=`.
3. Strip the query after it is applied so the URL does not keep a stale id across New chat. `router.replace(`/app/bots/${botId}`)` once the id is in state, matching the existing `?setup=1` strip.

## Chat layout

Install from `apps/www`:

```bash
pnpm dlx ai-elements@latest add conversation message tool
```

Add an `@ai-elements` registry entry to `apps/www/components.json` only if that CLI command refuses without one. Style remains `base-nova`. Dark theme stays `ProductThemeScope` / `html.dark`. Do not install `confirmation` or `prompt-input`. `NeedsYouCard` stays the interruption shell. `ChatComposerFrame` stays the composer, including attachments.

```tsx
// apps/www/components/app/workspace/chat-transcript.tsx
export function ChatTranscript(props: {
  children: ReactNode;
  scrollRef: RefObject<HTMLDivElement | null>;
  onScroll: () => void;
}): JSX.Element {
  throw new Error("not implemented");
  // Conversation + ConversationContent around today's max-w-3xl column
}

export function RunUserMessage(props: {
  text: string;
  sentAt?: string;
  extra?: ReactNode;
}): JSX.Element {
  throw new Error("not implemented");
  // Message from="user" wrapping current UserPromptBubble content
}

export function RunAssistantMessage(props: {
  children: ReactNode;
  leading?: ReactNode;
  footer?: ReactNode;
}): JSX.Element {
  throw new Error("not implemented");
  // Message from="assistant" wrapping current AssistantMessageBubble content
}

export function RunToolActivity(props: { tool: ToolActivity }): JSX.Element {
  throw new Error("not implemented");
  // Tool + ToolHeader + ToolContent. Map running/complete/error to Tool states.
  // Centered, not a chat bubble. Replaces RunActivityLine for tool-shaped items.
}
```

`bot-conversation-view.tsx` keeps run grouping, historical timelines, composer, onboarding, and failure cards. It swaps the overflow column for `ChatTranscript` and the two bubbles for `RunUserMessage` / `RunAssistantMessage`. It does not flatten runs into an AI SDK `UIMessage[]`.

`RunActivityLine` remains for non-tool text (`queued`, `host_restart`, permission deny).

## Module map

```
crates/agent-core/src/connectors.rs          unchanged ConnectorError
crates/agent-core/src/github_coding.rs       unchanged GithubCodingError
services/cloud-host/src/connectors/github_access.rs   classify_github_access
services/cloud-host/src/connector_need/service.rs     persist, emit, wait, notify
services/cloud-host/src/api/connectors.rs             returnTo on start/complete
services/cloud-host/src/connectors/db.rs              oauth return_to column
apps/www/lib/connector-need.ts                        parse, copy, returnTo
apps/www/lib/github-oauth.ts                          startGithubConnectorOAuth
apps/www/lib/run-event-timeline.ts                    connector + tool items
apps/www/contexts/active-run-context.tsx              RunActivityItem union
apps/www/components/app/connector-need-card.tsx
apps/www/components/app/workspace/run-conversation-timeline.tsx
apps/www/components/app/workspace/chat-transcript.tsx
apps/www/components/app/workspace/bot-conversation-view.tsx   Conversation restore
apps/www/app/app/connectors/github/callback/page.tsx
apps/www/components/ai-elements/{conversation,message,tool}.tsx  generated
```

Call chain for the card click, at most three files:

1. `connector-need-card.tsx` calls `startGithubConnectorOAuth`.
2. `github-oauth.ts` posts `/v1/connectors/github/oauth/start`.
3. Host stores `return_to` and returns `authorizeUrl`.

Call chain for resume after OAuth:

1. Callback posts `/v1/connectors/github/oauth/complete`.
2. `github_oauth_complete` upserts the credential and `notify_owner_github_changed`.
3. `request_and_wait` reclassifies and emits `connector_needed_resolved`. Dispatch retries.

No resume route. No `ask_user`. Tokens stay on the host.

## Tests

- `apps/www/lib/connector-need.test.ts`. Parse reasons. Reject bad `returnTo`. Copy for each reason kind.
- `apps/www/lib/run-event-timeline.test.ts`. Request then resolve. Duplicate `needId`. Replay after refresh. Do not turn a historical `tool_result` error string into a connector card.
- `apps/www/components/app/connector-need-card.test.tsx`. Pending CTA. Resolved has no Connect button. Distinct copy for reconnect vs unauthorized repo.
- `services/cloud-host/tests/connectors.rs`. `returnTo` round trip. Invalid `returnTo` is 400. Complete notifies waiters.
- New `services/cloud-host/tests/connector_need.rs`. Classify disconnected, reconnect, unauthorized repo, empty catalog. Idempotent second wait. OAuth that still omits the repo leaves the need pending.

UI proof is a real browser pass via `.cursor/skills/verify-elsewhere/` at `http://127.0.0.1:3000`. Scenario: owner with GitHub disconnected asks the bot to analyze a GitHub repo. Activity does not end on "Listing GitHub repositories did not complete" plus a URL ask. A Connect GitHub card appears. After OAuth the browser is on `/app/bots/:id` for that conversation and the run continues.
