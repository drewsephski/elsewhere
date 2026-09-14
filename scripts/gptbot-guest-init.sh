#!/bin/sh
set -e
mount -t proc proc /proc
mount -t sysfs sys /sys
mount -t devtmpfs dev /dev
mkdir -p /run /workspace
if ! grep -q ' / ' /proc/mounts; then
  mount /dev/vda / 2>/dev/null || true
fi
mkdir -p /workspace
/usr/local/bin/gptbot-guest-agent &

# Keep PID 1 alive so the VM stays up.
while true; do
  sleep 3600
done
