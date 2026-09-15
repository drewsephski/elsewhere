#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "== Frontend typecheck (root) =="
pnpm lint

echo "== www Next typegen + lint =="
pnpm --filter @elsewhere/www exec next typegen
pnpm --filter @elsewhere/www lint

echo "== Vitest (JS/TS unit tests) =="
pnpm test

echo "== www production build =="
pnpm build:www

echo "== Rust: agent-core =="
cargo test -p agent-core

echo "== Rust: cloud-host (requires Postgres via DATABASE_URL for sqlx tests) =="
if [[ -z "${DATABASE_URL:-}" ]]; then
  echo "DATABASE_URL is not set; skipping cloud-host integration tests."
  echo "Export DATABASE_URL (e.g. postgres://elsewhere:elsewhere@127.0.0.1:5432/elsewhere) for full coverage."
  cargo test -p cloud-host --features test-utils --lib
else
  cargo test -p cloud-host --features test-utils
fi

echo "All requested checks finished."
