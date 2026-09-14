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

echo "==> run 1: write hello.txt"
RUN1_JSON=$(post_run "e2e-live-1-$(date +%s)" "Create /workspace/hello.txt containing exactly:
hello from Elsewhere cloud

Then read the file and tell me exactly what it contains.")
RUN1_ID=$(echo "${RUN1_JSON}" | jq -r '.runId')
MODEL=$(echo "${RUN1_JSON}" | jq -r '.model')
echo "run1 id=${RUN1_ID} model=${MODEL}"
[ "${MODEL}" = "gpt-5.6-luna" ] || { echo "unexpected model"; exit 1; }

STATUS1=$(wait_for_run "${RUN1_ID}")
echo "run1 status=${STATUS1}"
[ "${STATUS1}" = "completed" ] || { curl -sf -H "${AUTH}" "${BASE}/v1/runs/${RUN1_ID}" | jq .; exit 1; }

EVENT_COUNT=$(curl -sf -H "${AUTH}" "${BASE}/v1/runs/${RUN1_ID}" | jq -r '.requestId' | xargs -I{} psql "${DATABASE_URL}" -tAc "SELECT COUNT(*) FROM run_events WHERE request_id='{}'")
echo "run1 durable events=${EVENT_COUNT}"
[ "${EVENT_COUNT}" -ge 3 ] || exit 1

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
