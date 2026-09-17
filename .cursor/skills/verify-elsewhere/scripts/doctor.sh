#!/usr/bin/env bash
set -euo pipefail

BASE="${ELSEWHERE_VERIFY_BASE:-http://127.0.0.1:3000}"
HOST="${ELSEWHERE_CLOUD_HOST_URL:-http://127.0.0.1:8080}"
TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT

code="$(curl -sS -o "$TMP" -w '%{http_code}' --max-time 5 "${BASE}/sign-in" || true)"
if [ "$code" != "200" ]; then
  echo "doctor: ${BASE}/sign-in returned HTTP ${code:-unreachable}"
  exit 1
fi

if ! grep -q 'Sign in' "$TMP"; then
  echo "doctor: ${BASE}/sign-in is not the Elsewhere sign-in page (missing heading Sign in)"
  exit 1
fi

if ! grep -qi 'elsewhere' "$TMP"; then
  echo "doctor: ${BASE}/sign-in is not the Elsewhere sign-in page (missing product name)"
  exit 1
fi

echo "web: ok  ${BASE}/sign-in HTTP 200"

host_code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 3 "${HOST}/health" || true)"
if [ "$host_code" = "200" ]; then
  echo "cloud-host: ok  ${HOST}/health HTTP 200"
else
  echo "cloud-host: down  ${HOST}/health HTTP ${host_code:-unreachable} (landing and sign-in recipes may still run)"
fi

runner_code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 3 "${BASE}/api/health/runner" || true)"
if [ "$runner_code" = "200" ]; then
  echo "runner: ok  ${BASE}/api/health/runner HTTP 200"
else
  echo "runner: down  ${BASE}/api/health/runner HTTP ${runner_code:-unreachable} (workspace recipes will fail)"
fi
