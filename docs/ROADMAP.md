# Elsewhere product roadmap

The north star is persistent ChatGPT-powered teammates, each with a computer, real assignments, visible progress, approval handoffs, routines, useful context, and finished results. See [PRODUCT_PROGRESS.md](PRODUCT_PROGRESS.md) for implementation checkpoints and proof boundaries.

## Implemented in the web product

- Better Auth accounts and owner-scoped resources.
- ChatGPT/Codex subscription engine, private persistent owner profiles, supported device connection, and explicit paid-API boundaries.
- AgentComputer terminal/files tools and persistent Fly Sprite computers.
- Durable work admission, queued dispatch, bot/computer serialization, cancellation, restart recovery, and replayable progress.
- Human approval lifecycle, expiry, cancellation, and a readable work timeline.
- Fixed-interval routines with atomic admission, overlap prevention, missed-occurrence coalescing, pause/resume, and failure review.
- Explicit editable bot context, versioned updates, and snapshots for future work.
- Immutable summaries and file results, private downloads, and per-work/result-library surfaces.
- Live workspace counts and bot presence; bot role/computer settings; runner readiness.

These are implementation claims with local regression coverage. They do not imply a deployed, authenticated, always-on subscription service.

## Next proof gates

1. Deploy one supervised runner with a persistent private profile volume and backed-up Postgres; configure Better Auth origins/JWKS, then verify a real owner device connection. No live credentials are checked into the repository.
2. Complete an approved real assignment, disconnect the client, and return to its result. Restart the host between queued tasks and verify recovery. Check subscription exhaustion/re-authentication behavior without paid fallback.
3. Add browser interaction behind AgentComputer and the existing approval contract, including screenshots, persistent browser state, safe navigation, and action evidence. Never expose the trusted host browser or credentials to a bot.
4. Add timezone-aware calendar schedules, longer task/checkpoint strategies, result retention/storage quotas, and operational alerts based on measured usage.
5. Improve contextual continuity from prior work with explicit provenance and controls. Current memory is owner-written context and persistent computer files, not automatic memory extraction.

## Deliberately later

Multi-bot handoffs, bot groups, semantic search, arbitrary skill packages, and collaboration follow a dependable single-bot product. Custom VM fleets, Kubernetes, GPUs, and generalized workflow engines are outside this scope.

## Desktop foundations retained

The Tauri app, SQLite persistence, macOS VM/guest-agent adapter, streaming chat, Keychain integration, and portable runtime remain intact. Production desktop viewing/browser interaction and updater work are separate from the current cloud product path.
