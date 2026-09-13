#!/usr/bin/env bash
set -euo pipefail

DISK_PATH="${1:?disk path required}"
ARCH_SLUG="${2:-aarch64}"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

DISK_DIR="$(dirname "$DISK_PATH")"
mkdir -p "$DISK_DIR"

if [[ -f "$DISK_PATH" ]] && [[ "$(stat -f%z "$DISK_PATH" 2>/dev/null || stat -c%s "$DISK_PATH")" -gt 10485760 ]]; then
  echo "Disk already exists at $DISK_PATH — skipping build"
  exit 0
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

ROOTFS_TAR="$WORK/alpine-minirootfs.tar.gz"
ROOTFS_NAME="$(
  curl -fsSL "https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/${ARCH_SLUG}/" \
    | rg -o "alpine-minirootfs-[0-9.]+-${ARCH_SLUG}\\.tar\\.gz" \
    | grep -v '_rc' \
    | sort -V \
    | tail -1
)"
if [[ -z "$ROOTFS_NAME" ]]; then
  echo "Could not resolve Alpine minirootfs for ${ARCH_SLUG}" >&2
  exit 1
fi
curl -fL "https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/${ARCH_SLUG}/${ROOTFS_NAME}" -o "$ROOTFS_TAR"

truncate -s 4G "$DISK_PATH"

TARGET="aarch64-unknown-linux-musl"
if [[ "$ARCH_SLUG" == "x86_64" ]]; then
  TARGET="x86_64-unknown-linux-musl"
fi

echo "Building guest agent + ext4 root disk via Docker ($TARGET)..."

docker run --rm --privileged \
  -v "$DISK_PATH:/disk.raw" \
  -v "$ROOTFS_TAR:/rootfs.tar.gz:ro" \
  -e ARCH_SLUG="$ARCH_SLUG" \
  -v "$REPO_ROOT/guest-agent:/guest-agent" \
  "rust:1-bookworm" bash -ec "
    set -euo pipefail
    apt-get update -qq
    apt-get install -y -qq gcc musl-tools e2fsprogs wget ca-certificates >/dev/null
    rustup target add $TARGET >/dev/null
    cd /guest-agent
    cargo build --release --target $TARGET
    AGENT=/guest-agent/target/$TARGET/release/gptbot-guest-agent

    cp /rootfs.tar.gz /tmp/alpine-minirootfs.tar.gz
    mkfs.ext4 -F /disk.raw
    mkdir -p /mnt
    mount /disk.raw /mnt
    tar -xzf /tmp/alpine-minirootfs.tar.gz -C /mnt
    mkdir -p /mnt/usr/local/bin /mnt/etc/init.d /mnt/workspace
    install -m 755 \"\$AGENT\" /mnt/usr/local/bin/gptbot-guest-agent

    mount --bind /dev /mnt/dev
    mount -t proc proc /mnt/proc
    mount -t sysfs sys /mnt/sys
    cp /etc/resolv.conf /mnt/etc/resolv.conf

    chroot /mnt /sbin/apk add --no-cache alpine-base e2fsprogs openrc >/dev/null

    cat > /mnt/etc/init.d/gptbot-guest-agent <<'EOF'
#!/sbin/openrc-run

name=\"gptbot-guest-agent\"
description=\"GPT Bot virtio guest agent\"

command=/usr/local/bin/gptbot-guest-agent
command_background=yes
pidfile=/run/gptbot-guest-agent.pid

depend() {
    need localmount
    after localmount
}
EOF
    chmod +x /mnt/etc/init.d/gptbot-guest-agent

    chroot /mnt /sbin/rc-update add gptbot-guest-agent default

    if ! grep -q '^/dev/vda ' /mnt/etc/fstab; then
      echo '/dev/vda / ext4 defaults 0 1' >> /mnt/etc/fstab
    fi

    umount /mnt/sys /mnt/proc /mnt/dev
    umount /mnt
  "

echo "Guest disk ready: $DISK_PATH"
