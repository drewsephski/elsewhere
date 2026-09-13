#!/usr/bin/env bash
# Phase 2A acceptance test — requires Docker, macOS, and Virtualization entitlement at runtime.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP_DATA="${GPTBOT_TEST_APP_DATA:-/tmp/gptbot-vm-acceptance}"
ENTITLEMENTS="$REPO_ROOT/src-tauri/entitlements.plist"
VMM_PKG="$REPO_ROOT/macos/gptbot-vmm"

rm -rf "$APP_DATA"
mkdir -p "$APP_DATA"

echo "== Build + sign gptbot-vmm =="
swift build -c release --package-path "$VMM_PKG"
VMM_BIN="$VMM_PKG/.build/release/gptbot-vmm"
codesign -f -s - --entitlements "$ENTITLEMENTS" --generate-entitlement-der "$VMM_BIN"
if ! codesign -dvvv --entitlements :- "$VMM_BIN" 2>&1 | rg -q 'com.apple.security.virtualization'; then
  echo "gptbot-vmm is missing com.apple.security.virtualization entitlement" >&2
  exit 1
fi
export GPTBOT_VMM_PATH="$VMM_BIN"
echo "Using signed gptbot-vmm: $GPTBOT_VMM_PATH"

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
