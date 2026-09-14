# Phase 3C.2 — Human tool approval

Mutating computer tools (`workspace_write`, `workspace_exec`) require durable human approval in cloud-host. Reads stay automatic.

## Policy

| Tool | Approval |
|------|----------|
| `workspace_list`, `workspace_read` | Auto-allow |
| `workspace_write`, `workspace_exec` | Human approval |
| Unknown tools | Treated as mutation (deny-by-approval) |

Approval is enforced in Elsewhere at the `AgentComputer` / MCP boundary (`ToolApprovalGate`), not via Codex prompts. Codex and Responses engines share the same gate via `SharedRunDeps`.

## Local / desktop

- JWT product runs: approvals always enforced.
- Internal-token / `legacy-local` runs: `AllowAllApprovalGate` unless `ELSEWHERE_ENFORCE_TOOL_APPROVALS=1`.

## APIs

- `GET /v1/approvals?status=pending`
- `GET /v1/approvals/:id`
- `POST /v1/approvals/:id/approve`
- `POST /v1/approvals/:id/deny`

Scoped by authenticated `owner_id` (404 cross-user).

## Config

- `ELSEWHERE_TOOL_APPROVAL_TIMEOUT_SECS` (default 300)
- `ELSEWHERE_ENFORCE_TOOL_APPROVALS` — enable approvals for internal-token E2E

## UI

- Bot chat: inline approval card on `approval_requested` SSE
- `/app/approvals` — pending list across runs

## Manual acceptance

See Phase 3C.2 spec: write `approval-proof.txt` (approve), deny `printf blocked` exec.
