#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../../.." && pwd)"
cd "$ROOT"

echo "==> Deploy runner (elsewhere-alpha-runner)"
fly deploy -c infra/fly/runner.toml --ha=false --strategy immediate

echo "==> Deploy web (elsewhere-alpha-web)"
fly deploy -c infra/fly/web.toml --ha=false --strategy immediate

echo "==> Smoke checks"
curl -sf "https://elsewhere-alpha-runner.fly.dev/ready" && echo "runner /ready OK"
curl -sf "https://elsewhere-alpha-web.fly.dev/api/auth/ok" && echo "web /api/auth/ok OK"
curl -sf "https://elsewhere-alpha-web.fly.dev/api/health/runner" && echo "web /api/health/runner OK"

echo "Done: https://elsewhere-alpha-web.fly.dev/"
