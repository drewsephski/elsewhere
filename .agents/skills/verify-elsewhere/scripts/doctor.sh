#!/usr/bin/env bash
set -euo pipefail

BASE="${ELSEWHERE_VERIFY_BASE:-http://127.0.0.1:3000}"
HOST="${ELSEWHERE_CLOUD_HOST_URL:-http://127.0.0.1:8080}"
TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT

fetch() {
  local url="$1"
  curl -sS -o "$TMP" -w '%{http_code}' --max-time 8 "$url" || true
}

# Landing is SSR-friendly and is the primary anonymous doctor target.
code="$(fetch "${BASE}/")"
if [ "$code" != "200" ]; then
  echo "doctor: ${BASE}/ returned HTTP ${code:-unreachable}"
  exit 1
fi
if ! grep -qi 'elsewhere' "$TMP"; then
  echo "doctor: ${BASE}/ is not Elsewhere (missing product name)"
  exit 1
fi
if ! grep -q 'Get started' "$TMP" && ! grep -qi 'open workspace' "$TMP"; then
  echo "doctor: ${BASE}/ missing expected landing CTAs (Get started / Open workspace)"
  exit 1
fi
echo "web: ok  ${BASE}/ HTTP 200 (landing)"

# Sign-in may be client-rendered (no "Sign in" in first HTML). Require 200 + Elsewhere.
code="$(fetch "${BASE}/sign-in")"
if [ "$code" != "200" ]; then
  echo "doctor: ${BASE}/sign-in returned HTTP ${code:-unreachable}"
  exit 1
fi
if ! grep -qi 'elsewhere' "$TMP"; then
  echo "doctor: ${BASE}/sign-in is not Elsewhere (missing product name)"
  exit 1
fi
if grep -q 'Sign in' "$TMP"; then
  echo "sign-in: ok  ${BASE}/sign-in HTTP 200 (SSR heading present)"
else
  echo "sign-in: ok  ${BASE}/sign-in HTTP 200 (client shell; confirm heading in browser)"
fi

host_code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 3 "${HOST}/health" || true)"
if [ "$host_code" = "200" ]; then
  echo "cloud-host: ok  ${HOST}/health HTTP 200"
else
  echo "cloud-host: down  ${HOST}/health HTTP ${host_code:-unreachable} (landing/sign-in recipes may still run; public runner DNS may be intentionally removed)"
fi

runner_code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 "${BASE}/api/health/runner" || true)"
if [ "$runner_code" = "200" ]; then
  echo "runner: ok  ${BASE}/api/health/runner HTTP 200"
else
  echo "runner: down  ${BASE}/api/health/runner HTTP ${runner_code:-unreachable} (workspace recipes will fail)"
fi
