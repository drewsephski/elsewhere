#!/usr/bin/env bash
# Phase 2A acceptance test — requires Docker (first provision), macOS, Virtualization entitlement.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FRESH=0
for arg in "$@"; do
  case "$arg" in
    --fresh) FRESH=1 ;;
    -h|--help)
      echo "Usage: $0 [--fresh]"
      echo "  default: reuse kernel, root.raw, and cached artifacts under GPTBOT_TEST_APP_DATA"
      echo "  --fresh: wipe test app data and prove cold-start provisioning"
      exit 0
      ;;
  esac
done

APP_DATA="${GPTBOT_TEST_APP_DATA:-/tmp/gptbot-vm-acceptance}"
ENTITLEMENTS="$REPO_ROOT/src-tauri/entitlements.plist"
VMM_PKG="$REPO_ROOT/macos/gptbot-vmm"

if [[ "$FRESH" -eq 1 ]]; then
  echo "== Fresh run: removing $APP_DATA =="
  rm -rf "$APP_DATA"
  export GPTBOT_FORCE_DISK_REBUILD=1
fi
mkdir -p "$APP_DATA"

DISK="$APP_DATA/vm/disks/root.raw"
DISK_GEN_STAMP="$APP_DATA/.guest-disk-generation"
EXPECTED_DISK_GEN="v2-minimal-init"
if [[ -f "$DISK" ]] && [[ ! -f "$DISK_GEN_STAMP" || "$(cat "$DISK_GEN_STAMP" 2>/dev/null)" != "$EXPECTED_DISK_GEN" ]]; then
  echo "== Guest disk generation mismatch (need $EXPECTED_DISK_GEN) — forcing rebuild =="
  export GPTBOT_FORCE_DISK_REBUILD=1
fi

echo "== Build + sign gptbot-vmm (incremental SwiftPM) =="
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

echo "== Provision (skips disk/kernel when already present) =="
run_rust provision
echo "$EXPECTED_DISK_GEN" > "$DISK_GEN_STAMP"

echo "== Start + wait guest + write proof =="
run_rust write-proof

echo "== Stop =="
run_rust stop

echo "== Fresh host process: start + wait guest + read proof =="
run_rust read-proof

echo "ACCEPTANCE PASSED (reuse=$([[ "$FRESH" -eq 1 ]] && echo false || echo true))"
