#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

pnpm dev:www &
WWW_PID=$!

cleanup() {
  kill "$WWW_PID" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

for _ in $(seq 1 60); do
  if curl -sf "http://127.0.0.1:3000/api/health/runner" >/dev/null 2>&1 \
    || curl -sf "http://127.0.0.1:3000/sign-in" >/dev/null 2>&1; then
    break
  fi
  sleep 0.5
done

pnpm dev
