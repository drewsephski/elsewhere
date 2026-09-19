# Rationale

## Problem

Bot chat is run-centric. The host already pauses a run for approvals and questions, then writes durable SSE that `applyRunStreamEvent` replays. GitHub access is the exception. Tools stay in the catalog, missing access becomes a `tool_result` error, and the loop continues. The model asks for a repo URL. That cannot authorize a repo (`github_open_repository` takes `owner` and `repo` only), and it cannot replace connecting the GitHub App. OAuth already exists at `POST /v1/connectors/github/oauth/start`, but the callback always lands on `/app/connectors`, so a chat user loses the conversation they were in. The layout refresh has to sit on the same run timeline. Chat cannot move to `useChat`. Tokens cannot enter the computer or the model. `ask_user` cannot be the connect card.

## Usage (caller's view)

See `USAGE.md`. The three callers stay small.

`RunConversationTimeline` renders `item.kind === "connector"` the way it renders `approval`. `ConnectorsManager.handleConnectGithub` and `ConnectorNeedCard` share `startGithubConnectorOAuth`. Host GitHub dispatch calls `classify_github_access`, then `request_and_wait`, then executes. Historical replay is `replayRunStreamEvents` on the same two event names. The UI never classifies `tool_result` text.

## Shape

**Data first.** `ConnectorNeed` is a discriminated union on `reason` plus a two-phase `status`. Disconnected, reconnect, unauthorized repo, and empty authorization are different values, not optional strings. `unauthorized_repo` cannot exist without `owner` and `repo`. `BotChatReturnTo` cannot be constructed except through `parseBotChatReturnTo`. That is `model-the-domain` and `type-system-discipline`.

**Flow.** Classify on the host before GitHub dispatch. `GithubAccess::Need` emits `connector_needed`, blocks the tool call, and waits. OAuth complete writes the owner credential, then notifies waiters. Each waiter reclassifies. If the reason is satisfied, emit `connector_needed_resolved` and retry the same tool. If the owner connected but still omitted the repo, the same `needId` stays pending. That is `make-operations-idempotent`. The chat card does not resume the run. Two writers would fight over run state. OAuth writes connector rows. The host owns waiters. Merge happens when the waiter reclassifies, per `separate-before-serializing-shared-state`.

**Parse at the boundary.** `parseConnectorNeed` is the only SSE adapter. `classify_github_access` is the only access adapter. Timeline components trust `ConnectorNeed`. No file matches `"Connector is not connected for this account"`. That is `boundary-discipline` and `encode-lessons-in-structure`.

**Approvals stay.** Connector need copies that interruption, it does not generalize it. No shared `RunInterruption` type. GitHub is the only provider in this design. `laziness-protocol`.

**Layout.** Install AI Elements `conversation`, `message`, and `tool` because they replace the scroll column, the two bubbles, and tool activity lines. Do not install `prompt-input`. `ChatComposerFrame` already owns attachments and send. Do not install `confirmation`. `NeedsYouCard` already owns approvals, questions, run failures, and this card. Wrapping those in a second confirmation primitive would be a pass-through. Do not flatten runs into `UIMessage[]`. `ChatTranscript` is presentational. `RunSummary` remains the record.

**Interface depth.** The public API is `classify_github_access` + `request_and_wait`, `parseConnectorNeed`, `startGithubConnectorOAuth`, and `ConnectorNeedCard`. Behind that sits persistence, the waiter registry, SSE, OAuth `return_to`, copy, and retry. Callers do not orchestrate those steps. Transport payloads stay inside the parsers.

**What this design does not do.** It does not hide GitHub tools until connect. It does not treat connection as a composer preflight. It does not infer a card from historical `tool_result` errors. It does not send tokens to the model. It does not use `ask_user`.

## Synthesis decision

Filled by orchestrator.

## Tradeoffs accepted

- We accept a new durable event pair and a `connector_need_requests` table in exchange for a wait the model cannot talk past. Without the wait, the screenshot failure stays: the assistant asks for a URL.
- We accept leaving GitHub tools in the catalog in exchange for discovering unauthorized repos at the tool that names `owner`/`repo`. Omitting tools cannot represent that case.
- We accept that old runs still replay "Listing GitHub repositories did not complete" in exchange for not inventing connector cards from stringly `tool_result` history.
- We accept not installing `prompt-input` and `confirmation` in exchange for keeping the composer and `NeedsYouCard` as the single interruption shell.
- We accept one pending need per run and provider in exchange for idempotent joins. A run that needed GitHub twice in a row still has one card.
- We accept an extra `?conversation=` round trip on OAuth return in exchange for restoring the exact thread. Conversation id is React state today, so `/app/bots/:id` alone is only "latest conversation."

## Alternatives considered

**Classify existing `tool_result` SSE in the client, no new protocol.** The public API is "inspect wire errors." Every caller learns `tool_denied` vs `malformed_tool_arguments` vs three English messages. Historical replay works, OAuth start can still take `returnTo`, and the diff is smaller. It lost because the agent does not wait. The loop continues and asks for a URL. The card would sit next to the bug, not stop it. Depth is inverted. Complexity leaks to the timeline.

**Omit GitHub tools until connected, and show a composer or empty-state card before send.** Connection becomes a preflight. Callers coordinate a catalog filter and an intent guess on the draft. That hides mid-run recovery, which is the case we have. It cannot distinguish unauthorized repo from disconnected without running a tool. Empty installs already return `[]` success, so the model can still ask for a URL after "connect." It lost because the product failure is mid-run, and a pasted URL cannot authorize a repo.

## Open questions and risks

- Group chat uses a different view. Should a group bot that hits GitHub render the same `connector` timeline item, and should `returnTo` stay `/app/bots/:id` or become `/app/groups/:id`?
- If classify treats an empty authorized catalog as `empty_authorization`, a user who connected GitHub and granted nothing will pause on list. Is that the intended empty-install behavior, or should list still return `[]` when the owner has chosen zero repos on purpose?
- Approval waits time out. What timeout should `request_and_wait` use, and should expiry look like the approval expired card?
- `UNIQUE (run_id, provider)` assumes one GitHub need at a time. If a future provider needs two concurrent needs in one run, that constraint is wrong. Is GitHub-only uniqueness enough?
- After OAuth, the waiter retries immediately. If GitHub's installation list lags the callback, reclassify may still see an unauthorized repo. Should the waiter retry classify a bounded number of times, or leave the card pending for another Connect click?

## Next implementation step

Add `ConnectorNeed` / `parseConnectorNeed` / `classify_github_access` and the `connector_needed` event pair with `not implemented` waits, then fill `request_and_wait` so a disconnected `github_list_repositories` blocks instead of emitting `tool_result` failure.
