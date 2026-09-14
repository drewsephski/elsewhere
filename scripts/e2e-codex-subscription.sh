#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

unset OPENAI_API_KEY
export ELSEWHERE_RUN_ENGINE=codex

if [ -f .env ]; then
  set -a
  # shellcheck disable=SC1091
  source .env
  set +a
fi

unset OPENAI_API_KEY

if [ -n "${OPENAI_API_KEY:-}" ]; then
  echo "OPENAI_API_KEY must be unset for Codex subscription gate" >&2
  exit 1
fi

export DATABASE_URL="${DATABASE_URL:-postgres://elsewhere:elsewhere@127.0.0.1:5432/elsewhere}"
: "${SPRITE_TOKEN:?SPRITE_TOKEN required}"
: "${ELSEWHERE_CLOUD_API_TOKEN:?ELSEWHERE_CLOUD_API_TOKEN required}"
export ELSEWHERE_TEST_SPRITE="${ELSEWHERE_TEST_SPRITE:-elsewhere}"

if ! command -v codex >/dev/null 2>&1; then
  echo "codex CLI is required for subscription E2E"
  exit 1
fi

export ELSEWHERE_BIND="${ELSEWHERE_BIND:-127.0.0.1:18081}"
BASE="http://${ELSEWHERE_BIND/0.0.0.0/127.0.0.1}"
AUTH="Authorization: Bearer ${ELSEWHERE_CLOUD_API_TOKEN}"
COMPUTER_ID="elsewhere-cloud-e2e"
SANDBOX_ID="sandbox-${COMPUTER_ID}"
EXPECTED_PROOF="hello from Elsewhere via ChatGPT subscription"

redact_secrets() {
  local line=$1
  line="${line//${ELSEWHERE_CLOUD_API_TOKEN}/[REDACTED_TOKEN]}"
  line="${line//${SPRITE_TOKEN}/[REDACTED_SPRITE_TOKEN]}"
  if [ -n "${OPENAI_API_KEY:-}" ]; then
    line="${line//${OPENAI_API_KEY}/[REDACTED_OPENAI_KEY]}"
  fi
  printf '%s\n' "${line}"
}

print_run_failure() {
  local run_id=$1
  local detail
  detail=$(curl -sf -H "${AUTH}" "${BASE}/v1/runs/${run_id}" | jq -c '{
    status: .status,
    model: .model,
    errorCode: .errorCode,
    assistantStatus: .assistantStatus,
    assistantResult: .assistantResult
  }')
  redact_secrets "${detail}" >&2
}

assert_terminal_status_ok() {
  local label=$1
  local status=$2
  local run_id=$3
  case "${status}" in
    completed) ;;
    failed|cancelled|interrupted|timeout)
      echo "${label} ended with status=${status}" >&2
      print_run_failure "${run_id}"
      exit 1
      ;;
    *)
      echo "${label} unexpected status=${status}" >&2
      print_run_failure "${run_id}"
      exit 1
      ;;
  esac
}

verify_sse_monotonic_ids() {
  local file=$1
  local prev=""
  while IFS= read -r line; do
    case "${line}" in
      id:*)
        local id="${line#id: }"
        id="${id// /}"
        if ! [[ "${id}" =~ ^[0-9]+$ ]]; then
          echo "SSE id is not numeric: ${id}" >&2
          return 1
        fi
        if [ -n "${prev}" ] && [ "${id}" -le "${prev}" ]; then
          echo "SSE ids not strictly increasing: ${prev} then ${id}" >&2
          return 1
        fi
        prev="${id}"
        ;;
    esac
  done < "${file}"
  if [ -z "${prev}" ]; then
    echo "SSE stream had no durable ids" >&2
    return 1
  fi
}

verify_sse_event_types() {
  local file=$1
  grep -q '^event: run_started$' "${file}" || {
    echo "SSE missing run_started" >&2
    return 1
  }
  grep -q '^event: tool_call$' "${file}" || {
    echo "SSE missing tool_call" >&2
    return 1
  }
  grep -q '^event: tool_result$' "${file}" || {
    echo "SSE missing tool_result" >&2
    return 1
  }
  grep -q '^event: terminal$' "${file}" || {
    echo "SSE missing terminal" >&2
    return 1
  }
  grep 'workspace_write\|workspace_read' "${file}" >/dev/null || {
    echo "SSE missing workspace tool activity" >&2
    return 1
  }
}

last_sse_durable_id() {
  local file=$1
  awk '/^id: / { sub(/^id: /,""); gsub(/ /,""); print }' "${file}" | tail -1
}

verify_sse_no_replay_below() {
  local file=$1
  local floor=$2
  while IFS= read -r line; do
    case "${line}" in
      id:*)
        local id="${line#id: }"
        id="${id// /}"
        if [ "${id}" -le "${floor}" ]; then
          echo "SSE reconnect replayed durable id ${id} (floor ${floor})" >&2
          return 1
        fi
        ;;
    esac
  done < "${file}"
}

verify_sandbox_row() {
  local row
  row=$(psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 -tAc \
    "SELECT id || '|' || provider || '|' || provider_resource_id FROM sandboxes WHERE id = '${SANDBOX_ID}'")
  if [ -z "${row}" ]; then
    echo "missing sandboxes row for ${SANDBOX_ID}" >&2
    exit 1
  fi
  IFS='|' read -r sid provider resource <<< "${row}"
  [ "${sid}" = "${SANDBOX_ID}" ] || exit 1
  [ "${provider}" = "fly_sprite" ] || exit 1
  [ "${resource}" = "${ELSEWHERE_TEST_SPRITE}" ] || {
    echo "sandbox provider_resource_id=${resource} expected ${ELSEWHERE_TEST_SPRITE}" >&2
    exit 1
  }
}

verify_run_started_metadata() {
  local request_id=$1
  local payload
  payload=$(psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 -tAc \
    "SELECT payload_json::text FROM run_events WHERE request_id = '${request_id}' AND event_type = 'run_started' ORDER BY id ASC LIMIT 1")
  if [ -z "${payload}" ]; then
    echo "missing run_started durable event" >&2
    exit 1
  fi
  echo "${payload}" | jq -e '.engine == "codex_subscription"' >/dev/null
  echo "${payload}" | jq -e '.model == "gpt-5.6-luna"' >/dev/null
  echo "${payload}" | jq -e '.provider.authType == "chatgpt"' >/dev/null
}

verify_mcp_tool_events() {
  local request_id=$1
  for tool in workspace_write workspace_read; do
    for kind in tool_call tool_result; do
      local count
      count=$(psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 -tAc \
        "SELECT COUNT(*) FROM run_events WHERE request_id = '${request_id}' AND event_type = '${kind}' AND payload_json->>'tool' = '${tool}' AND payload_json->>'server' = 'elsewhere'")
      if [ "${count}" -lt 1 ]; then
        echo "missing ${kind} for ${tool} (server=elsewhere)" >&2
        exit 1
      fi
    done
  done
}

verify_no_host_bypass() {
  local request_id=$1
  local host_count
  host_count=$(psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 -tAc \
    "SELECT COUNT(*) FROM agent_runs WHERE request_id = '${request_id}' AND error_code = 'host_tool_violation'")
  if [ "${host_count}" -gt 0 ]; then
    echo "host_tool_violation recorded for request ${request_id}" >&2
    exit 1
  fi
}

echo "==> Codex version: $(codex --version 2>/dev/null || true)"

echo "==> starting cloud-host (subscription path, no OPENAI_API_KEY)"
cargo run -q -p cloud-host > /tmp/elsewhere-cloud-host-codex.log 2>&1 &
HOST_PID=$!
SSE_FILE=$(mktemp)
SSE_RECONNECT_FILE=$(mktemp)
trap 'kill ${HOST_PID} 2>/dev/null || true; rm -f "${SSE_FILE}" "${SSE_RECONNECT_FILE}"' EXIT

for _ in $(seq 1 30); do
  if curl -sf "${BASE}/health" >/dev/null; then
    break
  fi
  sleep 1
done
curl -sf "${BASE}/health" >/dev/null || {
  echo "cloud-host failed to start; see /tmp/elsewhere-cloud-host-codex.log"
  exit 1
}

wait_for_run() {
  local run_id=$1
  for _ in $(seq 1 180); do
    status=$(curl -sf -H "${AUTH}" "${BASE}/v1/runs/${run_id}" | jq -r '.status')
    if [ "${status}" != "running" ]; then
      echo "${status}"
      return 0
    fi
    sleep 5
  done
  echo "timeout"
  return 1
}

post_run() {
  local idem=$1
  local message=$2
  curl -sf -X POST "${BASE}/v1/runs" \
    -H "${AUTH}" \
    -H "Idempotency-Key: ${idem}" \
    -H "Content-Type: application/json" \
    -d "$(jq -n \
      --arg msg "${message}" \
      --arg cid "${COMPUTER_ID}" \
      '{
        bot: { id: "codex-e2e-bot", name: "Codex E2E", instructions: "Use Elsewhere workspace tools only.", computerId: $cid, model: "gpt-5.6-luna" },
        message: $msg
      }')"
}

PROOF_PROMPT='Create /workspace/codex-subscription-proof.txt containing exactly:

hello from Elsewhere via ChatGPT subscription

Then read the file and tell me exactly what it contains.'

echo "==> proof run 1"
RUN1=$(post_run "codex-sub-proof-1-$(date +%s)" "${PROOF_PROMPT}")
RUN1_ID=$(echo "${RUN1}" | jq -r '.runId')
RUN1_REQUEST=$(echo "${RUN1}" | jq -r '.requestId')
MODEL=$(echo "${RUN1}" | jq -r '.model')
[ "${MODEL}" = "gpt-5.6-luna" ] || { echo "unexpected model ${MODEL}"; exit 1; }

verify_sandbox_row
echo "sandbox mapping ok: computerId=${COMPUTER_ID} sprite=${ELSEWHERE_TEST_SPRITE}"

curl -sfN -H "${AUTH}" "${BASE}/v1/runs/${RUN1_ID}/events" > "${SSE_FILE}" &
SSE_PID=$!

STATUS1=$(wait_for_run "${RUN1_ID}")
echo "run1 status=${STATUS1}"
assert_terminal_status_ok "run1" "${STATUS1}" "${RUN1_ID}"

for _ in $(seq 1 50); do
  grep -q '^event: terminal$' "${SSE_FILE}" && break
  sleep 0.2
done
grep -q '^event: terminal$' "${SSE_FILE}" || {
  echo "timed out waiting for terminal SSE event" >&2
  kill "${SSE_PID}" 2>/dev/null || true
  wait "${SSE_PID}" 2>/dev/null || true
  exit 1
}

kill "${SSE_PID}" 2>/dev/null || true
wait "${SSE_PID}" 2>/dev/null || true

verify_sse_monotonic_ids "${SSE_FILE}"
verify_sse_event_types "${SSE_FILE}"
LAST_SSE_ID=$(last_sse_durable_id "${SSE_FILE}")
echo "run1 sse last durable id=${LAST_SSE_ID}"

curl -sfN -H "${AUTH}" -H "Last-Event-ID: ${LAST_SSE_ID}" \
  "${BASE}/v1/runs/${RUN1_ID}/events" > "${SSE_RECONNECT_FILE}" &
SSE_RECONNECT_PID=$!
sleep 3
kill "${SSE_RECONNECT_PID}" 2>/dev/null || true
wait "${SSE_RECONNECT_PID}" 2>/dev/null || true
verify_sse_no_replay_below "${SSE_RECONNECT_FILE}" "${LAST_SSE_ID}"

verify_run_started_metadata "${RUN1_REQUEST}"
verify_mcp_tool_events "${RUN1_REQUEST}"
verify_no_host_bypass "${RUN1_REQUEST}"

RUN1_BODY=$(curl -sf -H "${AUTH}" "${BASE}/v1/runs/${RUN1_ID}")
RUN1_STATUS=$(echo "${RUN1_BODY}" | jq -r '.status')
RUN1_MODEL=$(echo "${RUN1_BODY}" | jq -r '.model')
RUN1_RESULT=$(echo "${RUN1_BODY}" | jq -r '.assistantResult // empty')
[ "${RUN1_STATUS}" = "completed" ] || exit 1
[ "${RUN1_MODEL}" = "gpt-5.6-luna" ] || exit 1
echo "${RUN1_RESULT}" | grep -qxF "${EXPECTED_PROOF}" || {
  echo "run1 assistantResult did not match expected proof text" >&2
  exit 1
}

PLAN_TYPE=$(psql "${DATABASE_URL}" -tAc \
  "SELECT payload_json->'provider'->>'planType' FROM run_events WHERE request_id = '${RUN1_REQUEST}' AND event_type = 'run_started' ORDER BY id ASC LIMIT 1")
AUTH_TYPE=$(psql "${DATABASE_URL}" -tAc \
  "SELECT payload_json->'provider'->>'authType' FROM run_events WHERE request_id = '${RUN1_REQUEST}' AND event_type = 'run_started' ORDER BY id ASC LIMIT 1")

echo "==> proof run 2 (read persistence)"
RUN2=$(post_run "codex-sub-proof-2-$(date +%s)" "Read /workspace/codex-subscription-proof.txt and tell me exactly what it contains.")
RUN2_ID=$(echo "${RUN2}" | jq -r '.runId')
STATUS2=$(wait_for_run "${RUN2_ID}")
echo "run2 status=${STATUS2}"
assert_terminal_status_ok "run2" "${STATUS2}" "${RUN2_ID}"

RUN2_RESULT=$(curl -sf -H "${AUTH}" "${BASE}/v1/runs/${RUN2_ID}" | jq -r '.assistantResult // empty')
echo "${RUN2_RESULT}" | grep -q "${EXPECTED_PROOF}" || {
  echo "run2 assistant did not contain expected file contents" >&2
  exit 1
}

if [ "${E2E_TEST_CANCEL:-}" = "1" ]; then
  echo "==> optional cancellation proof"
  CANCEL_RUN=$(post_run "codex-sub-cancel-$(date +%s)" "List every file under /workspace recursively with full paths, then read each file completely. Take your time and be thorough.")
  CANCEL_ID=$(echo "${CANCEL_RUN}" | jq -r '.runId')
  sleep 3
  curl -sf -X POST -H "${AUTH}" "${BASE}/v1/runs/${CANCEL_ID}/cancel" >/dev/null
  CANCEL_STATUS=$(wait_for_run "${CANCEL_ID}")
  if [ "${CANCEL_STATUS}" != "cancelled" ]; then
    echo "cancel run expected status=cancelled got ${CANCEL_STATUS}" >&2
    print_run_failure "${CANCEL_ID}"
    exit 1
  fi
  CANCEL_BODY=$(curl -sf -H "${AUTH}" "${BASE}/v1/runs/${CANCEL_ID}")
  echo "${CANCEL_BODY}" | jq -e '.assistantStatus == "cancelled"' >/dev/null
  CANCEL_REQ=$(echo "${CANCEL_BODY}" | jq -r '.requestId')
  TERM_COUNT=$(psql "${DATABASE_URL}" -tAc \
    "SELECT COUNT(*) FROM run_events WHERE request_id = '${CANCEL_REQ}' AND event_type = 'terminal'")
  [ "${TERM_COUNT}" -ge 1 ] || exit 1
  echo "optional cancellation proof passed"
fi

echo "E2E Codex subscription gate passed"
echo "  api_key_absent=true"
echo "  engine=codex_subscription authType=${AUTH_TYPE} planType=${PLAN_TYPE:-unknown}"
echo "  model=gpt-5.6-luna computerId=${COMPUTER_ID} sprite=${ELSEWHERE_TEST_SPRITE}"
