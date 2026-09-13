#!/usr/bin/env bash
# Phase 2A acceptance test — requires Docker, macOS, and Virtualization entitlement at runtime.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP_DATA="${GPTBOT_TEST_APP_DATA:-/tmp/gptbot-vm-acceptance}"
export GPTBOT_VMM_PATH="${GPTBOT_VMM_PATH:-$REPO_ROOT/macos/gptbot-vmm/.build/release/gptbot-vmm}"

rm -rf "$APP_DATA"
mkdir -p "$APP_DATA"

cd "$REPO_ROOT/src-tauri"

export RUST_LOG=gptbot=info

run_rust() {
  cargo run --quiet --example vm_acceptance -- "$APP_DATA" "$1"
}

echo "== Provision =="
run_rust provision

echo "== Start + write proof =="
run_rust write-proof

echo "== Stop =="
run_rust stop

echo "== Start again + read proof =="
run_rust read-proof

echo "ACCEPTANCE PASSED"
