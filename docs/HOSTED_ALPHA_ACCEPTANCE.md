# Hosted alpha acceptance — September 14, 2026

This report distinguishes deployed infrastructure from the laptop-off acceptance gate. The gate is **not yet passed**. No Responses API fallback is enabled, and no Codex OAuth files have been inspected or copied.

## Deployment inventory

- Supabase Free: `Elsewhere Alpha`, `edbfhcveqxtxnfybhmej`, us-east-2. Private web/runner roles and schemas, verified TLS, Data API disabled. See [database operations](SUPABASE_ALPHA.md).
- Fly web: `elsewhere-alpha-web`, one shared CPU / 512 MB Machine `e82d16e5c5d218`, ord. Public URL: https://elsewhere-alpha-web.fly.dev. Web autostop/autostart enabled; restart policy always.
- Web image: `registry.fly.io/elsewhere-alpha-web@sha256:9ff728ffee61980d39f86d063e6f29007bd59e905b654ae46c3e0e338d5b20fc`.
- Runner volume: `vol_r1j25gzdmxnw5z3r`, 3 GB in ord, encrypted, scheduled snapshots enabled with 7-day retention. Attached to runner Machine `683e91efe22398`, ord, one shared CPU / 2 GB RAM. Autostop disabled, restart always, SIGTERM grace 300 seconds.
- Runner image: `registry.fly.io/elsewhere-alpha-runner@sha256:695946c4adf085f5cd92807707e4c159fff3d5ce285ae8536399a998cee03855` (release 2, approval instructions). Fly runs its linux/amd64 manifest `sha256:dce63a7bf7dfc18cce6e34e1940277229e9d93f7c00d1a17d417ea19e24b3aa4`; same Machine and volume confirmed after update. Previous image index: `sha256:45a903c5bb7cd5dae91f8849a0be0f023b6248f0a43630d27d653dd1d4bfb4db`.

Approved scope is two Fly Machines, one encrypted 3 GB volume, and at most one new acceptance Sprite, using Fly hostnames. The existing Sprite named `elsewhere` remains a separate computer test resource. See [operations and cost controls](../infra/fly/README.md). The 48-hour test window is not an automatic shutdown timer.

## Evidence so far

- Linux web image built and deployed. Public `/sign-in`, `/api/auth/ok`, and `/api/auth/jwks` returned 200. Missing invitation was rejected with 403.
- After explicitly stopping the web Machine with SIGTERM, a public JWKS request woke it and returned 200 with its signing key in 8.91 seconds.
- The user created their real Elsewhere account and signed in through the hosted product. Browser verification showed their workspace.
- The isolated local web preflight verified valid signup, subsequent authenticated session, and invalid invitation rejection within 512 MB. These fixtures are not hosted acceptance data.
- Codex 0.154.0's generated app-server bundle does not contain its CLI MCP configuration schema. The previous schema-only build check failed. The corrected check separately verifies protocol methods and real CLI configuration parsing, checks the returned tool allow-list, and requires an invalid `omit_tools_from` value to fail. It does not use `app-server --help`, which can exit before configuration validation.

- Final Linux runner image passed both the protocol/configuration and device-schema/app-server checks. Local container preflight rejected a missing mount and a competing runner, reached readiness twice, exited cleanly on SIGTERM without OOM, and preserved a non-credential sentinel on its mounted directory across restart.
- Verification: 30 Codex-provider Rust tests, 12 frontend tests, both TypeScript/lint commands, Clippy (existing warnings), and Fly configuration validation passed. Remote GitHub CI has not been run.
- Conversational continuity: primary conversation per bot, Codex `thread/resume` with persisted `conversations.codex_thread_id`, bounded Responses history, and BFF header allow-listing (no Better Auth cookie forwarded to cloud-host). Set web `ELSEWHERE_CLOUD_HOST_INTERNAL_URL` to `http://elsewhere-alpha-runner.internal:8080` before removing public runner ingress.
- Password reset: production requires `RESEND_API_KEY`; reset email send is awaited and reset links are not logged in production.

- Hosted runner restarted successfully with its original volume after pairing. Fly CLI restart caps its explicit timeout at 60 seconds; a 300-second request was rejected before stopping the Machine, then the idle runner restarted with 60 seconds. The deployed Machine stop configuration remains SIGTERM/300 seconds. Active-work drain on Fly remains unproven.

## Real execution preflight

- Initial run `f55ccfb4-4a3d-43bc-b7e2-c91ea8e48c56` used `gpt-5.6-luna` but ended with a prose approval request and no tools. This was not accepted as task success. The follow-up explains that invoking a workspace tool creates the approval card; the execution policy now contains this instruction for future assignments.
- Successful run `ded56b74-8958-4cfb-95a3-5b7d2f7dd1df`: `gpt-5.6-luna`, five tool calls, 09:23:36–09:24:49 UTC. UI streamed read/write activity and approval cards without refresh. The earlier suspected stale-progress issue did not reproduce; no speculative streaming change was made.
- Bot `Alpha Proof`, ID `c512b43c-3364-430c-98b9-7a8cfab91502`, uses computer `Laptop-off acceptance`, Sprite `elsewhere-006aeead-181e-4088-9eb4-dc2241035247`. Only this one new Sprite was created.
- Real approval cards authorized the sentinel and result-file writes. The bot read back `/workspace/acceptance-sentinel.txt`, containing `ELSEWHERE-ALPHA-2026-09-14-PERSISTENCE` plus newline.
- Saved artifact `preflight.txt`, ID `0b810278-2f35-4fdd-891d-2693ae3a2c47`: 108 bytes, stored SHA-256 `0b2bbcac495fe7ba236d35eaeb743b43e4d68406a68bc6743b0aaf2c6aaa6d60`. Inspected the stored bytes; clicking its authenticated product link emitted a browser download event. The browser tool did not expose a local download path, so this hash is of the saved artifact, not a separately inspected downloaded file.
- Routine `Laptop-off sentinel check`, ID `e7f4763c-ce63-4b9c-bd8e-eda1683c5084`, is enabled for **09:40 UTC / 04:40 America/Chicago**, then every 24 hours. It reads the sentinel without mutation and saves a summary. Database and UI agree on its schedule; no occurrence has executed yet. Pause it after the acceptance occurrence unless the user wants ongoing daily checks.

## Laptop-off handoff

- Timed assignment `c78fc79e-7f4f-4b13-94cb-85fc2665bd6a` read the existing sentinel after the image update and local-service shutdown. Its command approval `4b034902-13ba-4fba-b938-a3956a630825` is pending and expires at **09:38:45 UTC / 04:38:45 Central**. The assignment requests one approved 50-second foreground command that records UTC start/end times and the existing sentinel in `laptop-off.txt`. The user should approve its action, observe it start, then immediately close the laptop. Return after **04:45 Central** in another browser/session and inspect this work and the 04:40 routine. The actual offline interval must be confirmed by the user and compared with durable timestamps.
- At **09:32:58 UTC**, local `elsewhere-web-preflight` and `dev-postgres-1` were stopped; localhost ports 1420, 3000, 8080, 18949, and 5432 were closed. Hosted readiness remained healthy. Local database/container data was retained.
- Pre-update opaque volume snapshot `vs_njAg8l59voZkS3GA7xOQ` completed at 09:27:47 UTC, retained seven days. No credential file was inspected/copied. Snapshot creation is proven; restore is not.

## Required live proof

| Gate | Evidence needed | Status |
| --- | --- | --- |
| Hosted readiness | One runner, database session lock, mounted encrypted volume, healthy dispatcher | Passed: `/ready` 200, database and dispatcher true; anonymous API 401; signed-in workspace loads |
| ChatGPT pairing | User completes product device authorization; Codex reports ChatGPT account | Passed: real product device flow; workspace reports Connected |
| Restart persistence | Same owner remains paired after hosted restart; real Luna work succeeds | Passed: pairing persisted after restart/full reload; real Luna tool assignment completed |
| Laptop-off work | Assignment completes after local services stop and laptop is offline | Pending |
| Laptop-off routine | Scheduled occurrence is admitted and completes while laptop is offline | Pending |
| Sprite persistence | Same computer/resource and known sentinel bytes survive later work | Passed across the runner image update: timed assignment read the original sentinel; laptop-off revisit still pending |
| Saved results | Fresh browser session reads durable progress and downloads exact artifact bytes | Current session: live results and browser download event passed; fresh-session/offline retrieval pending |
| Recovery | Graceful drain and claim exclusion; interrupted work never auto-replays | Local proof only |
| Backup restore | Opaque profile-volume snapshot and database backup restore drill | Pending |

Use a clearly named acceptance bot/computer and a non-sensitive assignment. Save a unique sentinel and output artifact through AgentComputer; obtain product approval for writes/commands. Observe a bounded long command before stopping local services. Record UTC timestamps for admission, local shutdown, completion, and return. Schedule the routine only after the initial work succeeds, with enough lead time for the user to disconnect. On return, use a separate browser session, inspect durable events, download the artifact, and verify the same Sprite sentinel. Do not infer physical laptop-off proof from a browser disconnect or a local fixture test.

Supabase Free has no automatic backups/PITR. A logical backup job and a restore drill remain required before widening the alpha to additional users. Volume snapshots alone do not back up accounts, queues, or saved artifact rows.
