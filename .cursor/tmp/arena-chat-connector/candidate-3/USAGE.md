# Usage — candidate 3 (caller's view)

Consumers are **`RunConversationTimeline`**, **`bot-conversation-view`**, and **`ConnectorsManager`**. They import domain helpers and OAuth start; they do not parse SSE payloads or tool errors.

---

## Quickstart

1. Host emits `connector_needed` / `connector_resolved` on the run event stream (same channel as approvals).
2. `applyRunStreamEvent` appends `RunActivityItem` entries with `kind: "connector"`.
3. Timeline renders `GithubConnectorCard`, which calls `startGithubConnectorOAuth({ returnTo })`.
4. After GitHub callback, user lands back on the bot conversation; the run unblocks and retries the blocked tool.

Install AI Elements components once (from `apps/www`):

```bash
pnpm dlx ai-elements@latest add conversation message tool
```

Wrap the transcript with `BotRunTranscript`; keep the existing composer.

---

## Call site 1 — timeline renders connector card like approval

`apps/www/components/app/workspace/run-conversation-timeline.tsx`

```tsx
import { GithubConnectorCard } from "@/components/app/github-connector-card";
import { isGithubConnectorNeed } from "@/lib/connector-need";

// inside items.map:
if (item.kind === "connector" && isGithubConnectorNeed(item.need)) {
  return (
    <GithubConnectorCard
      key={item.id}
      need={item.need}
      outcome={item.outcome}
      botId={item.need.botId}
    />
  );
}
```

The timeline does not branch on `tool_denied` or activity headlines.

---

## Call site 2 — bot conversation composes transcript + passes bot context

`apps/www/components/app/workspace/bot-conversation-view.tsx`

```tsx
import { BotRunTranscript } from "./bot-run-transcript";
import { BotRunMessage } from "./bot-run-message";
import { RunConversationTimeline } from "./run-conversation-timeline";
import { botConversationReturnTo } from "@/lib/github-oauth";

// Per run in the conversation list:
<BotRunTranscript runs={runs} activeRunId={activeRun?.runId ?? null} renderRun={(run) => (
  <>
    <BotRunMessage role="user">{/* existing user bubble content */}</BotRunMessage>
    <RunConversationTimeline
      items={timelineForRun(run.id)}
      botName={bot.name}
      pendingHumanIntervention={/* unchanged */}
    />
    <BotRunMessage role="assistant">{/* markdown + sources */}</BotRunMessage>
  </>
)} />

// Optional: after OAuth, URL may include ?connectorResume=1 — effect refreshes active run SSE once.
void botConversationReturnTo(bot.id, conversationId); // used by card + searchParams builder only
```

Run-centric ownership stays: one timeline per `run.id`, not a flat message list.

---

## Call site 3 — shared OAuth start (settings + in-chat)

`apps/www/components/app/connectors-manager.tsx`

```tsx
import { startGithubConnectorOAuth } from "@/lib/github-oauth";

async function handleConnectGithub() {
  setBusy(true);
  try {
    await startGithubConnectorOAuth({ returnTo: "/app/connectors" });
  } finally {
    setBusy(false);
  }
}
```

`apps/www/components/app/github-connector-card.tsx`

```tsx
import { startGithubConnectorOAuth, botConversationReturnTo } from "@/lib/github-oauth";

function handleConnect() {
  void startGithubConnectorOAuth({
    returnTo: botConversationReturnTo(botId, conversationId),
  });
}
```

Both paths hit `POST /v1/connectors/github/oauth/start` with an optional validated `returnTo`. Tokens never touch the browser beyond the GitHub redirect.

---

## What callers get back

| API | Returns |
|-----|---------|
| `connectorNeedFromPayload` | `ConnectorNeedPayload \| null` — safe parse at SSE boundary |
| `presentationForConnectorNeed` | Copy + tone for `NeedsYouCard` |
| `startGithubConnectorOAuth` | Redirects browser; no token in JS |
| `replayRunStreamEvents` | Same `kind: "connector"` items after refresh |

No caller imports `CloudEventSink`, SQL types, or `ToolError`.
