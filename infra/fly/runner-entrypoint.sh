#!/bin/sh
set -eu
umask 077
ulimit -c 0

# Refuse an ephemeral rootfs profile directory if the volume was not attached.
mountpoint -q /var/lib/elsewhere || {
  echo 'Runner refused startup: persistent profile volume is not mounted.' >&2
  exit 1
}
test "$ELSEWHERE_CODEX_PROFILES_DIR" = /var/lib/elsewhere/codex-profiles
install -d -m 0700 -o elsewhere -g elsewhere "$ELSEWHERE_CODEX_PROFILES_DIR"
gosu elsewhere test -w "$ELSEWHERE_CODEX_PROFILES_DIR"
exec gosu elsewhere /usr/local/bin/cloud-host
