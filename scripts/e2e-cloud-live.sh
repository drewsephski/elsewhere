#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

if [ -f .env ]; then
  set -a
  # shellcheck disable=SC1091
  source .env
  set +a
fi

export DATABASE_URL="${DATABASE_URL:-postgres://elsewhere:elsewhere@127.0.0.1:5432/elsewhere}"
: "${OPENAI_API_KEY:?OPENAI_API_KEY required}"
: "${ELSEWHERE_CLOUD_API_TOKEN:?ELSEWHERE_CLOUD_API_TOKEN required}"
: "${SPRITE_TOKEN:?SPRITE_TOKEN required}"

export ELSEWHERE_BIND="${ELSEWHERE_BIND:-127.0.0.1:18080}"
BASE="http://${ELSEWHERE_BIND/0.0.0.0/127.0.0.1}"
AUTH="Authorization: Bearer ${ELSEWHERE_CLOUD_API_TOKEN}"
COMPUTER_ID="elsewhere-cloud-e2e"

redact_secrets() {
  local line=$1
  line="${line//${ELSEWHERE_CLOUD_API_TOKEN}/[REDACTED_TOKEN]}"
  line="${line//${SPRITE_TOKEN}/[REDACTED_SPRITE_TOKEN]}"
  line="${line//${OPENAI_API_KEY}/[REDACTED_OPENAI_KEY]}"
  printf '%s\n' "${line}"
}

echo "==> starting cloud-host on ${ELSEWHERE_BIND}"
cargo run -q -p cloud-host > /tmp/elsewhere-cloud-host.log 2>&1 &
HOST_PID=$!
trap 'kill ${HOST_PID} 2>/dev/null || true' EXIT

for _ in $(seq 1 30); do
  if curl -sf "${BASE}/health" >/dev/null; then
    break
  fi
  sleep 1
done
curl -sf "${BASE}/health" >/dev/null || {
  echo "cloud-host failed to start; see /tmp/elsewhere-cloud-host.log"
  exit 1
}

wait_for_run() {
  local run_id=$1
  for _ in $(seq 1 120); do
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
        bot: { id: "e2e-bot", name: "E2E", instructions: "You are Luna. Use workspace tools.", computerId: $cid, model: "gpt-5.6-luna" },
        message: $msg
      }')"
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
  grep -q '^event: status$' "${file}" || {
    echo "SSE missing status" >&2
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

echo "==> run 1: write hello.txt"
RUN1_JSON=$(post_run "e2e-live-1-$(date +%s)" "Create /workspace/hello.txt containing exactly:
hello from Elsewhere cloud

Then read the file and tell me exactly what it contains.")
RUN1_ID=$(echo "${RUN1_JSON}" | jq -r '.runId')
MODEL=$(echo "${RUN1_JSON}" | jq -r '.model')
echo "run1 id=${RUN1_ID} model=${MODEL}"
[ "${MODEL}" = "gpt-5.6-luna" ] || { echo "unexpected model"; exit 1; }

SSE_FILE=$(mktemp)
trap 'kill ${HOST_PID} 2>/dev/null || true; rm -f "${SSE_FILE}" "${SSE_RECONNECT_FILE:-}"' EXIT

curl -sfN -H "${AUTH}" "${BASE}/v1/runs/${RUN1_ID}/events" > "${SSE_FILE}" &
SSE_PID=$!

STATUS1=$(wait_for_run "${RUN1_ID}")
echo "run1 status=${STATUS1}"

kill "${SSE_PID}" 2>/dev/null || true
wait "${SSE_PID}" 2>/dev/null || true

[ "${STATUS1}" = "completed" ] || {
  redact_secrets "$(curl -sf -H "${AUTH}" "${BASE}/v1/runs/${RUN1_ID}" | jq -c .)" >&2
  exit 1
}

verify_sse_monotonic_ids "${SSE_FILE}"
verify_sse_event_types "${SSE_FILE}"
LAST_SSE_ID=$(last_sse_durable_id "${SSE_FILE}")
echo "run1 sse last durable id=${LAST_SSE_ID}"

SSE_RECONNECT_FILE=$(mktemp)
curl -sfN -H "${AUTH}" -H "Last-Event-ID: ${LAST_SSE_ID}" \
  "${BASE}/v1/runs/${RUN1_ID}/events" > "${SSE_RECONNECT_FILE}" &
SSE_RECONNECT_PID=$!
sleep 3
kill "${SSE_RECONNECT_PID}" 2>/dev/null || true
wait "${SSE_RECONNECT_PID}" 2>/dev/null || true
verify_sse_no_replay_below "${SSE_RECONNECT_FILE}" "${LAST_SSE_ID}"
echo "run1 sse reconnect ok (no replay below ${LAST_SSE_ID})"

EVENT_COUNT=$(curl -sf -H "${AUTH}" "${BASE}/v1/runs/${RUN1_ID}" | jq -r '.requestId' | xargs -I{} psql "${DATABASE_URL}" -tAc "SELECT COUNT(*) FROM run_events WHERE request_id='{}'")
echo "run1 durable events=${EVENT_COUNT}"
[ "${EVENT_COUNT}" -ge 3 ] || exit 1

if [ -n "${ELSEWHERE_TEST_SPRITE:-}" ]; then
  echo "run1 sprite resource=${ELSEWHERE_TEST_SPRITE}"
fi

echo "==> run 2: read persistence"
RUN2_JSON=$(post_run "e2e-live-2-$(date +%s)" "Read /workspace/hello.txt and tell me exactly what it contains.")
RUN2_ID=$(echo "${RUN2_JSON}" | jq -r '.runId')
STATUS2=$(wait_for_run "${RUN2_ID}")
BODY=$(curl -sf -H "${AUTH}" "${BASE}/v1/runs/${RUN2_ID}" | jq -r '.assistantResult // empty')
echo "run2 status=${STATUS2}"
echo "${BODY}" | grep -q "hello from Elsewhere cloud" || {
  echo "run2 assistant did not contain expected file contents"
  exit 1
}

echo "==> host restart repair check"
psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 -c "INSERT INTO bots (id,name,system_prompt,model) VALUES ('restart-bot','r','','gpt-5.6-luna') ON CONFLICT DO NOTHING"
CONV=$(uuidgen | tr '[:upper:]' '[:lower:]')
ASST=$(uuidgen | tr '[:upper:]' '[:lower:]')
REQ="restart-req-$(uuidgen)"
psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 -c "INSERT INTO conversations (id,bot_id) VALUES ('${CONV}','restart-bot')"
psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 -c "INSERT INTO messages (id,conversation_id,role,kind,body,status,sequence) VALUES ('${ASST}','${CONV}','assistant','chat','','streaming',1)"
psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 -c "INSERT INTO agent_runs (id,request_id,bot_id,conversation_id,computer_id,model,status,assistant_message_id,step_count,started_at) VALUES ('$(uuidgen)','${REQ}','restart-bot','${CONV}','${COMPUTER_ID}','gpt-5.6-luna','running','${ASST}',0,NOW())"

kill "${HOST_PID}" || true
wait "${HOST_PID}" 2>/dev/null || true
sleep 2
cargo run -q -p cloud-host > /tmp/elsewhere-cloud-host-restart.log 2>&1 &
HOST_PID=$!

for _ in $(seq 1 30); do
  curl -sf "${BASE}/health" >/dev/null && break
  sleep 1
done

MSG_STATUS=$(psql "${DATABASE_URL}" -tAc "SELECT status FROM messages WHERE id='${ASST}'")
RUN_STATUS=$(psql "${DATABASE_URL}" -tAc "SELECT status FROM agent_runs WHERE request_id='${REQ}'")
echo "restart repair message=${MSG_STATUS} run=${RUN_STATUS}"
[ "${MSG_STATUS}" = "interrupted" ] && [ "${RUN_STATUS}" = "interrupted" ] || exit 1

echo "==> run 3 after restart (filesystem still present)"
RUN3_JSON=$(post_run "e2e-live-3-$(date +%s)" "Read /workspace/hello.txt and tell me exactly what it contains.")
RUN3_ID=$(echo "${RUN3_JSON}" | jq -r '.runId')
STATUS3=$(wait_for_run "${RUN3_ID}")
BODY3=$(curl -sf -H "${AUTH}" "${BASE}/v1/runs/${RUN3_ID}" | jq -r '.assistantResult // empty')
echo "run3 status=${STATUS3}"
echo "${BODY3}" | grep -q "hello from Elsewhere cloud" || exit 1

echo "E2E cloud live gate passed"
