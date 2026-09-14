# Hosted alpha acceptance — September 14, 2026

This report distinguishes **shipped code evidence**, **local/CI verification**, and **live hosted gates**. Hosted alpha is **not accepted** until every row in the final pass/fail table is green. No Responses API fallback is enabled, and no Codex OAuth files have been inspected or copied.

## Closure pass (preview cache + refresh coalescing)

| Area | Change | Status |
| --- | --- | --- |
| Browser preview cache | Content-addressed JPEGs under `preview/frames/<etag>.jpg`; composite ETag includes URL/title + image bytes; image written first, `meta.json` renamed last; bounded frame retention | Shipped + `browser-preview-cache.test.mjs` |
| Browser preview read path | Daemon and `read_browser_preview_cache` load metadata first, then the referenced frame file only (no `latest.jpg`) | Shipped |
| Preview refresh UX | Single-flight fetches with at most one dirty follow-up; abort only on computer change, disable, unmount, or timeout; polling uses same path | Shipped + `browser-preview-fetch-scheduler.test.ts` |
| Browser egress | `click`/`type` inside temporary egress window; snapshot/preview reads offline | On `main` + unit tests |
| Computer registry | Owner/archive revalidation, eviction, provider resource replacement | On `main` + `computer_registry_lifecycle` |
| SSE replay | Durable `assistant_delta` catch-up via `Last-Event-ID` | `assistant_stream_replay` |
| Runner routing audit | Server-side Next.js cloud-host calls use `cloudHostUpstreamBaseUrl()` | Verified in repo |
| Web Docker build | `Dockerfile.web` installs build toolchain for native deps | Shipped (uncommitted until push) |
| Public runner ingress | Still enabled in `infra/fly/runner.toml` until BFF smoke passes | **Not removed yet** |

### Local verification (this workspace)

| Check | Result |
| --- | --- |
| `pnpm test` (vitest, incl. preview cache + scheduler) | **Pass** |
| `pnpm lint` + `@elsewhere/www` `tsc` + `next build` | **Pass** |
| `cargo test -p sprite-computer` | **Pass** |
| `cargo test -p cloud-host --features test-utils` | **Pass** |
| `cargo test -p agent-core -p computer-mcp -p codex-provider` | **Pass** (when run with cloud-host batch) |
| GitHub Actions on current `main` head | **Pending** — acceptance commit not pushed (protected `main`; operator push required) |

## Deployment inventory

- Supabase Free: `Elsewhere Alpha`, `edbfhcveqxtxnfybhmej`, us-east-2. Private web/runner roles and schemas, verified TLS, Data API disabled. See [database operations](SUPABASE_ALPHA.md).
- Fly web: `elsewhere-alpha-web`, one shared CPU / 512 MB Machine `e82d16e5c5d218`, ord. Public URL: https://elsewhere-alpha-web.fly.dev. Web autostop/autostart enabled; restart policy always.
- Runner volume: `vol_r1j25gzdmxnw5z3r`, 3 GB in ord, encrypted, scheduled snapshots enabled with 7-day retention. Attached to runner Machine `683e91efe22398`, ord, one shared CPU / 2 GB RAM. Autostop disabled, restart always, SIGTERM grace 300 seconds.
- Set web `ELSEWHERE_CLOUD_HOST_INTERNAL_URL` to `http://elsewhere-alpha-runner.internal:8080` before removing public runner ingress.

Approved scope is two Fly Machines, one encrypted 3 GB volume, and at most one new acceptance Sprite, using Fly hostnames. The existing Sprite named `elsewhere` remains a separate computer test resource. See [operations and cost controls](../infra/fly/README.md).

## Evidence so far (hosted)

- Linux web image built and deployed. Public `/sign-in`, `/api/auth/ok`, and `/api/auth/jwks` returned 200 (last check 2026-09-14). Missing invitation rejected with 403.
- User created real Elsewhere account and signed in through hosted product.
- Real Luna tool assignment completed on acceptance bot/computer; pairing persisted across runner restart.
- Sprite persistence sentinel proved across runner image update (`ELSEWHERE-ALPHA-2026-09-14-PERSISTENCE`).
- Saved artifact download verified in-session; fresh-session/offline retrieval still pending laptop-off gate.
- Local runner drain tests exist; Fly active-work drain still unproven in this pass.

## Live gates (required before Connected Apps Phase 1)

| Gate | Evidence needed | Status |
| --- | --- | --- |
| GitHub CI green on release head | All CI jobs on pushed commit | **Pending push** |
| Deploy runner/web with this pass | Browser bootstrap v3 + preview cache module on acceptance Sprite | **Pending deploy** |
| Hosted BFF smoke | Full signed-in flow via `/api/cloud/*`; confirm private `.internal` runner path | **Pending** |
| Remove public runner ingress | After BFF smoke; re-smoke web-only | **Pending** |
| Browser acceptance (Luna) | example.com → snapshot → click → form type → screenshot/result; default-deny restored | **Pending** |
| Preview after tool results | Live preview frames update after successful browser tools | **Pending** |
| Latency (`elsewhere_run_phases`) | Text Luna, browser Luna, Codex follow-up TTFT breakdown | **Pending measurement** |
| Laptop-off assignment | 60–90s assignment completes while laptop offline | **Pending — user** |
| Laptop-off routine | Read-only routine fires while laptop offline | **Pending — user** |
| Fresh session | Durable history, artifact download, same Sprite sentinel | **Pending** |
| DB logical backup + restore | Isolated verification target | **Pending** |
| Profile volume snapshot restore | Opaque restore; no OAuth file inspection | **Pending** |
| Production password reset | Resend flow with `RESEND_API_KEY` | **Pending** |
| Graceful drain on Fly | Interrupted work never auto-replays | **Pending** |

## Latency measurement (operator)

After deploying this pass, capture `elsewhere_run_phases` logs (`RUST_LOG` includes target) for:

1. One text-only Luna turn (`gpt-5.6-luna`)
2. One browser/tool Luna turn
3. One Codex thread follow-up on an existing thread

Report phases: admission → claim, claim → Codex launch, launch → thread open/resume, thread open → turn start, turn start → first model delta, first delta → durable `assistant_delta`, estimated user-visible TTFT, completion.

**Do not** implement Codex process pooling unless measurements show launch dominates controllable TTFT; run-specific MCP credentials make naive reuse unsafe.

## Fresh laptop-off acceptance (September 14, 2026 — attempt R2)

Do **not** use the expired 04:40 routine or assignment `c78fc79e-7f4f-4b13-94cb-85fc2665bd6a`.

### Staging (operator — product UI)

Create on the acceptance bot/computer (same Sprite as persistence sentinel):

1. **Foreground assignment (60–90s, non-sensitive)**  
   - Prompt (example): run a foreground shell command that sleeps **75 seconds**, then writes exactly `ELSEWHERE-LAPTOPOFF-2026-09-14-R2` to `/workspace/laptopoff-r2-sentinel.txt` and confirms with `cat`.  
   - Approve any exec approval when prompted.  
   - Note the new run id when queued.

2. **Read-only routine (first run ≥10–15 minutes after staging)**  
   - Name: `Laptop-off sentinel check R2`  
   - Interval: daily (or minimum 15 minutes for a one-off test window — use **first run** at least 15 minutes after you finish staging).  
   - Suggested first run (America/Chicago): **2026-09-14 09:52:00 CDT** (adjust if staging completes later; must be ≥15 minutes after assignment is admitted).  
   - Task: read-only — verify `/workspace/laptopoff-r2-sentinel.txt` still contains `ELSEWHERE-LAPTOPOFF-2026-09-14-R2` and append a line `routine-r2-ok` to `/workspace/laptopoff-r2-routine.log` (no destructive commands).

3. **Deploy** uncommitted preview-cache pass to runner/web before closing the laptop so bootstrap version **3** is on the acceptance Sprite.

### Your checklist (physical laptop-off)

Times below assume staging completes near **2026-09-14 09:36 CDT**; shift routine return time if you stage later.

| Step | Local clock (America/Chicago) | Action |
| --- | --- | --- |
| 1 | After assignment shows **running** | Approve exec if still pending |
| 2 | **09:37–09:38** | Confirm run started, then **close the laptop** (sleep/off — not just browser) |
| 3 | Stay offline until after **09:53 CDT** | Allows ~75s assignment + routine at **09:52** |
| 4 | **09:54+** | Open **fresh browser profile** (or sign out → sign in) |
| 5 | Verify | Assignment **completed** while offline; routine occurrence **completed**; `/workspace/laptopoff-r2-sentinel.txt` bytes; result download if collected; browser preview/history replay without duplicate assistant text |
| 6 | After verification | Pause routine `Laptop-off sentinel check R2` unless ongoing checks desired |

Record artifact SHA-256 after download in this doc when known.

## Pass/fail summary

| Gate | Pass? |
| --- | --- |
| Preview cache transactional consistency (code + unit tests) | **Pass** |
| Preview refresh single-flight (code + unit tests) | **Pass** |
| Local CI-equivalent tests | **Pass** |
| GitHub CI on pushed head | **Fail** (not pushed) |
| Deploy + hosted BFF smoke | **Fail** (not run this session) |
| Public runner removed + re-smoke | **Fail** |
| Luna browser acceptance on hosted stack | **Fail** |
| Latency measurements published | **Fail** |
| Laptop-off assignment + routine | **Fail** (awaiting user) |
| Recovery drills (DB + volume + drain + password reset) | **Fail** |

**Hosted alpha accepted:** **No** — remaining gates are deploy, live smoke, latency, laptop-off, recovery, and CI on pushed head.

**Next product phase when all green:** Connected Apps Phase 1 (do not start until this table is all Pass).
