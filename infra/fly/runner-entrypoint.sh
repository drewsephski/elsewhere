#!/bin/sh
set -eu
umask 077
ulimit -c 0

# Refuse an ephemeral rootfs profile directory if the volume was not attached.
mountpoint -q /var/lib/elsewhere || {
  echo 'Runner refused startup: persistent profile volume is not mounted.' >&2
  exit 1
}

# Every durable profile root (Codex/ChatGPT credentials, Chromium browser sign-in state) must
# sit on the volume, be pinned to the path infra/fly/runner.toml declares, exist with mode 0700
# owned by the service user, and be writable once privileges drop. cloud-host only creates the
# per-profile subdirectories; it cannot create the roots after `gosu elsewhere`.
prepare_profile_root() {
  name=$1
  actual=$2
  expected=$3
  if [ "$actual" != "$expected" ]; then
    echo "Runner refused startup: $name must be $expected (got '$actual')." >&2
    exit 1
  fi
  install -d -m 0700 -o elsewhere -g elsewhere "$expected"
  gosu elsewhere test -w "$expected" || {
    echo "Runner refused startup: $expected is not writable by the elsewhere user." >&2
    exit 1
  }
}

prepare_profile_root ELSEWHERE_CODEX_PROFILES_DIR "${ELSEWHERE_CODEX_PROFILES_DIR:-}" /var/lib/elsewhere/codex-profiles
prepare_profile_root ELSEWHERE_BROWSER_PROFILES_DIR "${ELSEWHERE_BROWSER_PROFILES_DIR:-}" /var/lib/elsewhere/browser-profiles
exec gosu elsewhere /usr/local/bin/cloud-host
