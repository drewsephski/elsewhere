# Synthesis — chat layout + GitHub connector need

## Pick

Base: candidate-1. Cross-judge [Cross-judge](d2f0dfe2-1639-4865-b09b-1eeea2f06275) scored 30/30 vs C2 22 and C3 25. Parent agrees.

All three runners chose shape A (durable `connector_needed`). That is consensus. Differences were classification timing, post-OAuth resolve, and AI Elements scope.

## Why candidate-1

Classify happens before GitHub dispatch (`classify_github_access` → `GithubAccess::Need`). OAuth complete notifies waiters. Each waiter reclassifies. If the repo is still unauthorized, the same `needId` stays pending. `BotChatReturnTo` is branded. Copy comes from `reason.kind`. AI Elements are `conversation`, `message`, and `tool` only.

## Grafts

- From C2: optional `blockedInvocationId` on the need payload so a tool activity line can correlate with the gate.
- From C3: `presentationForConnectorNeed` as the name of the copy helper (durable `ConnectorNeed` vs `NeedsYouCard` props).
- From C3: GitHub mark on the card if a logo already exists in brand assets. Do not invent a new icon set.

## Rejected

- C2 as base: `prompt-input` around the composer, `ToolError` intercept, SSE `message` copy, `resolve_connected` without reclassify.
- C3 as base: `resolve_on_github_connected` bulk-resolve, `connectorResume=1` client poll, agent-core `ConnectorNeedGate` trait.
- Shape B and C as primary: agent talks past the card; preflight cannot represent unauthorized repo.

## Contract

Implement `.cursor/tmp/arena-chat-connector/candidate-1/SKETCH.md` with the grafts above. Do not invent a second runtime model.
