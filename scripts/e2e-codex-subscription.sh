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
COMPUTER_ID="${ELSEWHERE_TEST_SPRITE}"

echo "==> Codex version: $(codex --version 2>/dev/null || true)"

echo "==> starting cloud-host (subscription path, no OPENAI_API_KEY)"
cargo run -q -p cloud-host > /tmp/elsewhere-cloud-host-codex.log 2>&1 &
HOST_PID=$!
trap 'kill ${HOST_PID} 2>/dev/null || true' EXIT

for _ in $(seq 1 30); do
  if curl -sf "${BASE}/health" >/dev/null; then
    break
  fi
  sleep 1
done
curl -sf "${BASE}/health" >/dev/null

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
STATUS1=$(wait_for_run "${RUN1_ID}")
echo "run1 status=${STATUS1}"
echo "${RUN1}" | jq '{runId, requestId, status}'

echo "==> proof run 2 (read persistence)"
RUN2=$(post_run "codex-sub-proof-2-$(date +%s)" "Read /workspace/codex-subscription-proof.txt and tell me exactly what it contains.")
RUN2_ID=$(echo "${RUN2}" | jq -r '.runId')
STATUS2=$(wait_for_run "${RUN2_ID}")
echo "run2 status=${STATUS2}"

echo "==> SSE sample (run 1)"
curl -sf -N -H "${AUTH}" "${BASE}/v1/runs/${RUN1_ID}/events" | head -n 40 || true

echo "==> done (verify assistant output and tool events in Postgres manually if needed)"
