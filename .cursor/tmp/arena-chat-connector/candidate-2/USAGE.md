# Usage (caller's view)

Consumers integrate through three surfaces: timeline replay, live SSE, and the bot run thread layout. No caller parses GitHub error strings.

## Quickstart

1. Install AI Elements primitives used by the layout shell (from `apps/www`):

```bash
pnpm dlx ai-elements@latest add conversation message prompt-input
```

2. Import domain types and reducers from one module:

```ts
import {
  type ConnectorNeededPayload,
  connectorNeedFromStreamPayload,
} from "@/lib/connector-need";
import { applyRunStreamEvent, replayRunStreamEvents } from "@/lib/run-event-timeline";
```

3. Render connector cards in the timeline next to approvals:

```tsx
import { ConnectorNeedCard } from "@/components/app/connector-need-card";
```

4. Start GitHub OAuth with a bot-scoped return URL (shared with Connectors settings):

```ts
import { startGithubConnectorOAuth } from "@/lib/connectors/github-oauth";
```

---

## Call site 1 — `RunConversationTimeline`

Timeline stays a dumb switch on `RunActivityItem.kind`. Connector needs are first-class items, same as approvals.

```tsx
// apps/www/components/app/workspace/run-conversation-timeline.tsx
import { ConnectorNeedCard } from "@/components/app/connector-need-card";

{items.map((item) => {
  if (item.kind === "connector") {
    return (
      <ConnectorNeedCard
        key={item.id}
        payload={item.connector}
        externalPhase={item.phase}
        botId={botId}
      />
    );
  }
  if (item.kind === "approval") {
    return <ApprovalCard key={item.id} payload={item.approval} externalStatus={item.decision} />;
  }
  // ... question, subagent, text unchanged
})}
```

**Returns:** Nothing async. Card owns OAuth navigation and optional “Continue run” when phase is still `pending` after OAuth.

---

## Call site 2 — Historical replay (`replayRunStreamEvents`)

Same reducer path as live SSE. Refreshing `/app/bots/:id` replays durable `connector_needed` / `connector_resolved` events into identical cards.

```ts
// apps/www/lib/historical-run-timeline-loader.ts (conceptual)
const events = await fetchRunEvents(runId);
const { items } = replayRunStreamEvents(runId, events);
// items may include { kind: "connector", connector: ConnectorNeededPayload, phase: "connected" }
```

**Returns:** `RunTimelineState` with connector items interleaved in event order (typically after the activity line that triggered the gate).

---

## Call site 3 — `BotConversationView` run thread (AI Elements)

Each `RunSummary` becomes one turn inside a run-centric thread. Active run uses live timeline + assistant stream; history uses `useHistoricalRunTimelines`.

```tsx
// apps/www/components/app/workspace/bot-conversation-view.tsx
import { BotRunThread } from "@/components/app/bot-chat/bot-run-thread";

<BotRunThread
  runs={conversationRuns}
  liveRunId={activeRunId}
  getTimeline={(runId) => (runId === activeRunId ? liveTimeline.items : historical[runId]?.items ?? [])}
  getAssistantMarkdown={(run) => /* existing snippet / stream */}
  composer={<BotChatComposer botId={botId} onSubmit={handleSend} />}
/>
```

**Returns:** Scrollable conversation layout (AI Elements `Conversation` + `Message` roles). Composer remains the existing attachment-aware composer wrapped in `PromptInput` chrome only where it replaces custom styling—not `useChat`.

---

## OAuth return

Chat card and Connectors settings both call:

```ts
await startGithubConnectorOAuth({
  returnTo: buildBotConversationReturnUrl({ botId, conversationId, runId }),
});
```

After GitHub App install, callback reads `state.returnTo` and `router.replace(returnTo)` so the user lands on the same bot conversation with `?connector=github&runId=…` (optional) for post-connect resume hints.
