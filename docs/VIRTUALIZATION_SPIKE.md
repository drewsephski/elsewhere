# Phase 2A — Local Computer virtualization spike

## Goal

Prove **Elsewhere** can own a **persistent Linux VM** on macOS via **Virtualization.framework**, talk to a **guest agent** over **Virtio sockets**, and survive stop / app restart without recreating the disk.

## Approaches considered

| Approach | Verdict |
|----------|---------|
| Unsafe Rust → Virtualization.framework bindings | Rejected — high maintenance, poor fit with VZ APIs |
| Firecracker / QEMU / Docker-as-VM | Rejected per product direction |
| **Swift helper + Rust `VirtualMachineManager`** | **Chosen** — narrow native boundary, official VZ API |
| Go `vz` library from Rust | Rejected — extra runtime + CGO without clear win |

## Chosen architecture

```text
React (VM diagnostics panel)
  ↓ Tauri IPC
Rust VirtualMachineManager (src-tauri/src/vm/)
  ↓ Unix socket JSON control plane
gptbot-vmm (Swift, macos/gptbot-vmm/)
  ↓ Virtualization.framework
Alpine Linux guest (persistent root.raw)
  ↓ AF_VSOCK :1024
gptbot-guest-agent (Rust, guest-agent/)
```

### Why a Swift subprocess?

`VZVirtualMachine` and `VZVirtioSocketDevice` are Objective-C/Swift APIs. A small **`gptbot-vmm`** executable keeps Rust free of fragile FFI while still letting the Rust layer own lifecycle, paths, and provisioning.

Control protocol (host Rust ↔ Swift): newline-delimited JSON on `vm/runtime/vmm.sock`.

Guest protocol: newline-delimited JSON RPC (`ping`, `exec`, `read_file`, `write_file`, `list_dir`), version field `protocolVersion: 1`.

## Host architecture

- Detected at runtime via `std::env::consts::ARCH` (`arm64` / `x86_64`).
- Guest arch matches host arch for this spike (no cross-arch Linux on Apple Silicon).

## Guest image strategy

- **Kernel**: Pinned **Kata Containers** `vmlinux-6.12.36-160` from `kata-static-3.19.1-*` (ARM64 Linux **Image** magic — required by `VZLinuxBootLoader`; Alpine `vmlinuz-virt` PE stubs are rejected).
- **Root disk**: 4 GiB `ext4` on `vm/disks/root.raw`, built by `scripts/build-guest-disk.sh` (Docker + musl `gptbot-guest-agent`).
- **PID1**: Minimal `/sbin/init` script (`scripts/gptbot-guest-init.sh`) that `exec`s `gptbot-guest-agent`; OpenRC remains on disk as fallback with `gptbot-guest-agent` in **sysinit**.
- **Cmdline** (in `vm.json`): `console=hvc0 root=/dev/vda rw rootfstype=ext4 rootwait init=/sbin/gptbot-init`

## VM storage layout

Under app data (`~/Library/Application Support/com.drewsepeczi.gptbot/`):

```text
vm/
  config/vm.json
  disks/root.raw
  artifacts/linux-image
  runtime/vmm.sock
  logs/console.log
  logs/vmm.log
```

## Entitlements

`src-tauri/entitlements.plist`: `com.apple.security.virtualization` = true

**Local dev:** sign the VMM helper after SwiftPM build:

```bash
swift build -c release --package-path macos/gptbot-vmm
codesign -f -s - --entitlements src-tauri/entitlements.plist --generate-entitlement-der \
  macos/gptbot-vmm/.build/release/gptbot-vmm
export GPTBOT_VMM_PATH="$PWD/macos/gptbot-vmm/.build/release/gptbot-vmm"
```

## Lifecycle (Phase 2A)

- **`start`**: bring up `VZVirtualMachine` only; returns `running` without blocking on the guest agent.
- **`wait_guest`**: poll vsock `ping` with host-side timeouts (Rust control socket + Swift `SO_RCVTIMEO` / `SO_SNDTIMEO`).
- **`stop`**: stop VM and tear down the VMM process.

On guest wait failure, Rust/Swift append **`console.log` tail** to the error.

## Development loop

```bash
./scripts/vm-acceptance-test.sh          # reuse kernel, disk, signed VMM when possible
./scripts/vm-acceptance-test.sh --fresh  # wipe test app data + cold provision
```

`GPTBOT_FORCE_DISK_REBUILD=1` is honored by `provision.rs` (not only by the shell script). Guest disk generation stamp: `v3-exec-init` under `GPTBOT_TEST_APP_DATA/.guest-disk-generation`.

## Known limitations

- **Docker required** for guest disk builds.
- **No desktop / Chromium** — headless Alpine only.
- **Single VM** (`default` id).
- **Kata kernel** without external initrd: root must come up on `/dev/vda` directly; guest tuning is sensitive to PID1/OpenRC interaction.
- **Entitlement**: VM start fails clearly if virtualization entitlement is missing.
- Guest `exec` cannot be interrupted cleanly from the host yet.

## Acceptance test

Script: `scripts/vm-acceptance-test.sh` → `cargo run --example vm_acceptance`.

Proof file: `/workspace/proof.txt` with contents `hello from the persistent Elsewhere computer`.

### Results (2026-09-14, this workspace)

| Step | Status | Notes |
|------|--------|-------|
| Harness reuse / `--fresh` | **Verified** | Incremental SwiftPM + conditional disk rebuild |
| Kata kernel + ext4 validation | **Verified** | Rejects zero-filled / non-ext4 disks |
| VMM `start` / `wait_guest` split | **Verified** | Swift `start` no longer waits on guest |
| Control socket timeouts | **Verified** | Rust read/write deadlines; Swift guest vsock timeouts |
| VM boot + guest `ping` | **Blocked** | Console shows OpenRC at PID1 on some boots; guest vsock not ready within 120s |
| Write/read proof + persistence | **Pending** | Blocked on guest readiness |

**Blockers observed**

1. **`GPTBOT_FORCE_DISK_REBUILD` ignored by Rust** until fixed in `provision.rs` (disk reuse skipped rebuild while iterating init).
2. **Guest PID1**: Even with `/sbin/init` replaced on `root.raw`, serial console still showed OpenRC during failed runs — likely stale boots before rebuild; continue validating with `v3-exec-init` disk generation and `vmm.log` cmdline logging.
3. **OpenRC hang** at “Caching service dependencies …” when OpenRC is PID1 — mitigations: custom init + sysinit service + `rc_depend_strict="NO"`.

Re-run after disk generation `v3-exec-init`:

```bash
export GPTBOT_FORCE_DISK_REBUILD=1
./scripts/vm-acceptance-test.sh
./scripts/vm-acceptance-test.sh --fresh
```

Record `ACCEPTANCE PASSED` lines here when green.
