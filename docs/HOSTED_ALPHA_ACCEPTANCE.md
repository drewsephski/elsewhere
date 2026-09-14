# Hosted alpha acceptance — September 14, 2026

This report distinguishes deployed infrastructure from the laptop-off acceptance gate. The gate is **not yet passed**. No Responses API fallback is enabled, and no Codex OAuth files have been inspected or copied.

## Closure pass (this branch)

Reliability and correctness fixes landed for hosted alpha:

| Area | Change | Status |
| --- | --- | --- |
| Browser egress | `click` and `type` run inside the same serialized temporary-egress window as `navigate`/`download`; snapshot/preview-cache reads stay offline | Shipped in code + unit tests; live `ELSEWHERE_BROWSER_SMOKE=1` still required |
| Computer registry | Every access re-validates owner + non-archived sandbox; archive evicts cache; provider resource changes replace stale entries; idle/size-bounded eviction | Shipped + `computer_registry_lifecycle` tests |
| Browser preview ETag | Daemon persists monotonic version, SHA-256 `etag`, atomic meta/image writes; cloud-host uses content `etag` for `If-None-Match` | Shipped |
| Preview refresh UX | Refresh on successful `tool_result` only; queue one follow-up if generation changes mid-fetch; 30s recovery poll unchanged | Shipped |
| SSE replay | Integration test: durable `assistant_delta` catch-up via `Last-Event-ID`, commentary vs answer, terminal `fullContent` | `assistant_stream_replay` test |
| Runner routing audit | All server-side Next.js cloud-host calls use `cloudHostUpstreamBaseUrl()` (private Fly URL when set) | Verified in repo |
| Public runner ingress | Still enabled in `infra/fly/runner.toml` until hosted BFF smoke passes | **Not removed yet** |

## Deployment inventory

- Supabase Free: `Elsewhere Alpha`, `edbfhcveqxtxnfybhmej`, us-east-2. Private web/runner roles and schemas, verified TLS, Data API disabled. See [database operations](SUPABASE_ALPHA.md).
- Fly web: `elsewhere-alpha-web`, one shared CPU / 512 MB Machine `e82d16e5c5d218`, ord. Public URL: https://elsewhere-alpha-web.fly.dev. Web autostop/autostart enabled; restart policy always.
- Runner volume: `vol_r1j25gzdmxnw5z3r`, 3 GB in ord, encrypted, scheduled snapshots enabled with 7-day retention. Attached to runner Machine `683e91efe22398`, ord, one shared CPU / 2 GB RAM. Autostop disabled, restart always, SIGTERM grace 300 seconds.
- Set web `ELSEWHERE_CLOUD_HOST_INTERNAL_URL` to `http://elsewhere-alpha-runner.internal:8080` before removing public runner ingress.

Approved scope is two Fly Machines, one encrypted 3 GB volume, and at most one new acceptance Sprite, using Fly hostnames. The existing Sprite named `elsewhere` remains a separate computer test resource. See [operations and cost controls](../infra/fly/README.md).

## Evidence so far

- Linux web image built and deployed. Public `/sign-in`, `/api/auth/ok`, and `/api/auth/jwks` returned 200. Missing invitation was rejected with 403.
- User created real Elsewhere account and signed in through hosted product.
- Real Luna tool assignment completed on acceptance bot/computer; pairing persisted across runner restart.
- Sprite persistence sentinel proved across runner image update (`ELSEWHERE-ALPHA-2026-09-14-PERSISTENCE`).
- Saved artifact download verified in current session; fresh-session/offline retrieval still pending.
- Local runner drain tests exist; Fly active-work drain still unproven.

## Required live proof

| Gate | Evidence needed | Status |
| --- | --- | --- |
| Hosted readiness | One runner, database session lock, mounted encrypted volume, healthy dispatcher | Passed |
| ChatGPT pairing | User completes product device authorization | Passed |
| Restart persistence | Pairing + Luna work after hosted restart | Passed |
| Laptop-off work | Assignment completes after local services stop and laptop is offline | **Pending — user** |
| Laptop-off routine | Scheduled occurrence completes while laptop offline | **Pending — user** |
| Sprite persistence | Same computer and sentinel across later work | Passed (offline revisit still pending) |
| Saved results | Fresh browser session downloads exact artifact bytes | Partial |
| Recovery | Graceful drain; interrupted work never auto-replays | Local only |
| Backup restore | Logical DB backup + restore drill; Codex profile volume snapshot restore (no OAuth inspection) | **Pending** |
| Browser acceptance | Real `gpt-5.6-luna`: navigate → snapshot → click → type → screenshot/result with default-deny restored | Code ready; live smoke pending |
| BFF-only runner | Full hosted smoke through `/api/cloud/*` then remove public runner ingress | **Pending smoke** |
| Latency (`elsewhere_run_phases`) | Text Luna, browser Luna, Codex follow-up turn TTFT breakdown | **Pending measurement on deploy** |

## Laptop-off checklist (for you)

Complete when timed assignment `c78fc79e-7f4f-4b13-94cb-85fc2665bd6a` is ready:

1. Approve the pending command approval for the 50s foreground timing command (if still pending).
2. Confirm assignment starts, then **close the laptop** (not just the browser).
3. Leave offline until after **04:45 America/Chicago** so routine `Laptop-off sentinel check` (04:40) can fire.
4. Return in a **fresh browser session** (new profile or signed-out → signed-in).
5. Verify durable run events, routine occurrence, sentinel bytes, and artifact download hash `0b2bbcac495fe7ba236d35eaeb743b43e4d68406a68bc6743b0aaf2c6aaa6d60`.
6. Pause the daily routine afterward unless you want ongoing checks.

## Latency measurement (operator)

After deploying this pass, capture `elsewhere_run_phases` logs for:

1. One text-only Luna turn
2. One browser/tool Luna turn
3. One Codex thread follow-up

Report: admission → claim, claim → Codex launch, launch → thread open, thread open → turn start, turn start → first model delta, first delta → durable `assistant_delta`, estimated TTFT, total completion. If process launch dominates TTFT, document numbers before any pooling design (run-specific MCP creds make naive reuse unsafe).

## Remaining blockers before widening alpha

- Laptop-off assignment + routine (physical)
- DB logical backup + restore drill
- Codex profile volume snapshot restore verification (opaque)
- Production password-reset email with `RESEND_API_KEY`
- Hosted BFF smoke → remove public runner ingress
- Live browser + latency gates on deployed stack
