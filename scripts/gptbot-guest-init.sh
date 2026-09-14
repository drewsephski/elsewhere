#!/bin/sh
set -e
echo "gptbot-init: starting PID1" >/dev/console
mount -t proc proc /proc
mount -t sysfs sys /sys
mount -t devtmpfs dev /dev
mkdir -p /run /workspace /var/log
if ! grep -q ' / ' /proc/mounts; then
  mount /dev/vda / 2>/dev/null || true
fi
mkdir -p /workspace
echo "gptbot-init: launching guest agent" >/dev/console
exec /usr/local/bin/gptbot-guest-agent
