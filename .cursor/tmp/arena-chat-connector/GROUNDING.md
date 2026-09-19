# Grounding: chat layout + GitHub connector card

Read-only facts from how explorers. Do not treat this as a design. Produce your own candidate.

## Product ask

Improve the bot chat transcript layout using AI Elements (conversation, message, prompt-input, tool, confirmation as appropriate). When the bot intends to work on GitHub and GitHub is not connected or not installed for the needed accounts, show an in-thread connector card. The user should be able to sign in / install the GitHub App from that card so the bot can work on repos.

Screenshot failure today: user asked DrewOS to analyze the Elsewhere repo. Activity shows "Listing GitHub repositories did not complete". Assistant asks for a repo URL. That cannot replace connecting GitHub.

## Architecture facts

- Chat is run-centric, not AI SDK `useChat`. Each `RunSummary` is one user bubble + timeline + assistant reply.
- Shell: `apps/www/components/app/workspace/workspace-shell.tsx`. Center: `bot-conversation-view.tsx`. Timeline: `run-conversation-timeline.tsx`.
- SSE: `GET /v1/runs/:id/events` → `applyRunStreamEvent` in `apps/www/lib/run-event-timeline.ts`.
- `RunActivityItem.kind`: `"text" | "approval" | "subagent" | "question"` only.
- Shared interruption shell: `NeedsYouCard` (`pending` | `resolved` | `neutral`). Approvals compact. Questions and `RunFailureCard` full size.
- AI Elements already in `apps/www/components/ai-elements/`: shimmer, sources, suggestion, attachments. Not installed: conversation, message, prompt-input, tool, confirmation.
- Package runner: `pnpm`. Install via `pnpm dlx ai-elements@latest add <name>` from `apps/www`. `components.json` style `base-nova`. No `@ai-elements` registry entry yet. Add registry if the CLI requires it.
- GitHub is owner-scoped GitHub App user-token (`ghu_`/`ghr_`). Connect: `POST /v1/connectors/github/oauth/start` → `https://github.com/apps/{slug}/installations/new?state=`. Complete: `/app/connectors/github/callback` currently `router.replace("/app/connectors")`.
- Tokens never enter the computer. Host tarball checkout + Git Data API publish.
- GitHub tools always in the model catalog. Missing access discovered at execution. Runtime continues the loop. Run does not fail.
- Read tools not connected: `ToolError::Denied`, `errorCode: tool_denied`, message `"Connector is not connected for this account"`.
- Coding tools not connected: `ToolError::MalformedArguments`, `errorCode: malformed_tool_arguments`, message `"Connect GitHub in Connectors before working on a repository."`
- Reconnect: `ReconnectRequired`. Unauthorized repo (connected, not in install catalog): `"GitHub repository is not in the authorized installation set"`. Empty installs: list returns `[]` success.
- `ask_user` is 2-4 multiple choice and rejects OAuth/secret solicitation.
- `github_open_repository` takes `owner` + `repo` only. A pasted URL cannot authorize a repo.
- ChatGPT missing uses terminal `RunFailureCard` + `runRecoveryGuide` (`codex_not_authenticated`). GitHub is mid-run, not terminal.
- Onboarding copy: setup never enables connectors.
- Tests: Vitest next to components (`approval-card.test.tsx`). Rust tests in `services/cloud-host/tests/connectors.rs`.
- Verify UI via `.cursor/skills/verify-elsewhere/` in a real browser. Default local `http://127.0.0.1:3000`.

## Constraints the design must honor

1. Do not rewrite chat onto `useChat` / AI SDK UIMessage.
2. Do not put GitHub tokens on the computer or in the model.
3. Do not use `ask_user` as the connect card.
4. OAuth start already exists. Reuse `POST /v1/connectors/github/oauth/start`.
5. Owner-scoped connection, not per-bot.
6. Preserve existing approval / question / human-intervention cards.
7. Dark product theme (`ProductThemeScope`, `html.dark`).
8. Comments only for non-obvious why. No phase-narrating comments.

## Callers that must stay simple

- `RunConversationTimeline` should render a connector card the way it renders `ApprovalCard`.
- `ConnectorsManager.handleConnectGithub` should remain the OAuth start implementation or share one function with the chat card.
- Historical replay (`replayRunStreamEvents`) must show the same card after refresh.
