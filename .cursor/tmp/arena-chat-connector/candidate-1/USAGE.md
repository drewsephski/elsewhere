# Caller's view

Chat stays run-centric. Each `RunSummary` is still one user turn, a timeline, and an assistant reply. You do not adopt `useChat`.

When a GitHub tool cannot run because the owner has no usable GitHub App access, the host pauses that tool the same way it pauses an approval. The timeline grows a `connector` item. The card sends the owner through existing GitHub App OAuth and lands them back on `/app/bots/:botId` for this conversation. After OAuth completes, the host retries the same tool. The model never sees a "paste a repo URL" error for a missing connection.

## What you import

```ts
import { ConnectorNeedCard } from "@/components/app/connector-need-card";
import { startGithubConnectorOAuth } from "@/lib/github-oauth";
import {
  botChatReturnTo,
  parseBotChatReturnTo,
  parseConnectorNeed,
  type ConnectorNeed,
} from "@/lib/connector-need";
import { Conversation, ConversationContent } from "@/components/ai-elements/conversation";
import { Message, MessageContent } from "@/components/ai-elements/message";
import { Tool, ToolHeader, ToolContent } from "@/components/ai-elements/tool";
```

Host code imports `classify_github_access` and `ConnectorNeedService::request_and_wait`. There is no second client API for "resume the run."

## Call site 1. Timeline render

`RunConversationTimeline` treats a connector item the way it treats an approval. It does not read tool error strings.

```tsx
export function RunConversationTimeline({
  items,
  botName,
  pendingHumanIntervention,
  returnTo,
}: RunConversationTimelineProps) {
  return (
    <>
      {pendingHumanIntervention ? (
        <HumanInterventionBanner pending={pendingHumanIntervention} />
      ) : null}
      {items.map((item) => {
        if (item.kind === "approval") {
          return (
            <ApprovalCard
              key={item.id}
              payload={item.approval}
              externalStatus={item.decision}
            />
          );
        }
        if (item.kind === "connector") {
          return (
            <ConnectorNeedCard
              key={item.id}
              need={item.need}
              returnTo={returnTo}
            />
          );
        }
        if (item.kind === "tool") {
          return <RunToolActivity key={item.id} tool={item.tool} />;
        }
        // subagent, question, text: unchanged
      })}
    </>
  );
}
```

`applyRunStreamEvent` is the only parser. Historical replay is the same reducer.

```ts
if (streamEvent.event === "connector_needed") {
  const need = parseConnectorNeed(runId, payload);
  if (!need) return state;
  if (items.some((item) => item.kind === "connector" && item.need.needId === need.needId)) {
    return state;
  }
  return { items: [...items, { id, kind: "connector", need }], pendingHumanIntervention };
}

if (streamEvent.event === "connector_needed_resolved") {
  return {
    items: items.map((item) =>
      item.kind === "connector" && item.need.needId === payload.needId
        ? { ...item, need: { ...item.need, status: parseConnectorNeedStatus(payload) } }
        : item,
    ),
    pendingHumanIntervention,
  };
}
```

## Call site 2. Connect from the card, and from Integrations

Both buttons call one function. The chat card passes a bot-chat return path. The Integrations page omits it and keeps today's landing on `/app/connectors`.

```ts
// connector-need-card.tsx
async function handleConnect() {
  const { authorizeUrl } = await startGithubConnectorOAuth({ returnTo });
  window.location.href = authorizeUrl;
}

// connectors-manager.tsx
async function handleConnectGithub() {
  const { authorizeUrl } = await startGithubConnectorOAuth();
  window.location.href = authorizeUrl;
}
```

Build `returnTo` from the bot and conversation the card is sitting in.

```ts
const returnTo = botChatReturnTo({ botId, conversationId });
// "/app/bots/bot_123?conversation=conv_456"
```

The callback page reads `returnTo` from the complete response. It does not hardcode `/app/connectors`.

```ts
const body = await response.json() as ConnectorCompleteResponse;
router.replace(body.returnTo ?? "/app/connectors");
```

On remount, `BotConversationView` reads `?conversation=` before falling back to the latest conversation for that bot.

## Call site 3. Host dispatch (the wait)

GitHub tools stay in the catalog. Missing access is no longer a `tool_result` the model has to interpret. Classify, wait, retry.

```rust
pub async fn dispatch_github_tool(...) -> Result<Value, ConnectorError> {
    loop {
        match classify_github_access(&self.pool, owner_id, tool_name, arguments).await? {
            GithubAccess::Ready => return self.execute_github_tool(...).await,
            GithubAccess::Need(reason) => {
                self.connector_needs
                    .request_and_wait(ConnectorNeedRequest {
                        owner_id,
                        run_id,
                        bot_id,
                        tool_name,
                        reason,
                    })
                    .await?;
                // loop: reclassify. OAuth may still omit the needed repo.
            }
        }
    }
}
```

`github_oauth_complete` upserts the credential, then notifies every pending GitHub need for that owner. The waiter reclassifies. If the reason is now satisfied, it emits `connector_needed_resolved` and returns. The UI does not call a resume endpoint.

Dismiss or timeout returns `ConnectorNeedError::Dismissed`. The tool then returns today's `ToolError` so the model can stop. That path is the exception, not the screenshot case.
