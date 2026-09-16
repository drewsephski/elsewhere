# Elsewhere product roadmap

The north star is persistent ChatGPT-powered teammates, each with a computer, real assignments, visible progress, approval handoffs, routines, useful context, and finished results. See [PRODUCT_PROGRESS.md](PRODUCT_PROGRESS.md) for implementation checkpoints and proof boundaries.

## Implemented in the web product

- Better Auth accounts, invitation-gated hosted signup, and owner-scoped resources.
- ChatGPT/Codex subscription engine, private persistent owner profiles, supported device connection, and explicit paid-API boundaries.
- AgentComputer terminal, files, and browser tools on persistent Fly Sprite computers, with browser sign-in profiles stored on the runner volume.
- Durable work admission, queued dispatch, bot/computer serialization, cancellation, restart recovery, and replayable progress.
- Human approval lifecycle plus human browser takeover, expiry, cancellation, and a readable work timeline.
- Timezone-aware scheduled Routines and event-triggered Routines (generic incoming webhook), both using the same occurrence, Bot, computer, Skill snapshot, approval, result, and history path.
- Owner-authored Skills with version snapshots, GitHub connector, bot groups, @mentions, and one-way Bot-to-Bot handoff.
- Explicit editable bot context, versioned updates, and snapshots for future work.
- Immutable summaries and file results, private downloads, and per-work/result-library surfaces.
- Live workspace counts and bot presence; bot role/computer settings; runner readiness.

These are implementation claims with local regression coverage. Hosted alpha exists; it does not imply a multi-tenant always-on production service.

## Genuine remaining gaps

1. Additional event sources and connectors beyond the generic webhook and GitHub OAuth connector. Deployment-system examples should keep using the webhook URL rather than a second execution engine.
2. Result retention/storage quotas, operational alerts, and longer task/checkpoint strategies based on measured usage.
3. Automatic memory extraction from prior work. Current memory is owner-written context, Skills, and persistent computer files.
4. Automatically waking a source Bot when a delegated teammate finishes; automatic cross-computer artifact transfer beyond bounded text and explicit result files.
5. Semantic search across work/results, and treating Skills as arbitrary untrusted packages from the public internet.

Live ChatGPT pairing, laptop-off completion, volume restore, and subscription exhaustion behavior still need ongoing operational proof on the hosted runner. No live credentials are checked into the repository.

## Deliberately later

Custom VM fleets, Kubernetes, GPUs, and a generalized workflow engine are outside this scope. Event-triggered Routines are a thin admission path onto existing work, not a workflow product.

## Desktop foundations retained

The Tauri app, SQLite persistence, macOS VM/guest-agent adapter, streaming chat, Keychain integration, and portable runtime remain intact. Production desktop viewing/browser interaction and updater work are separate from the current cloud product path.
