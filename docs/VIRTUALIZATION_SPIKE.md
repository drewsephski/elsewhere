# Phase 2A — Local Computer virtualization spike

## Goal

Prove Elsewhere can own a **persistent Linux VM** on macOS via **Virtualization.framework**, talk to a **guest agent** over **Virtio sockets**, and survive stop / app restart without recreating the disk.

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

Guest protocol: newline-delimited JSON RPC (`ping`, `exec`, `read_file`, `write_file`), version field `protocolVersion: 1`.

## Host architecture

- Detected at runtime via `std::env::consts::ARCH` (`arm64` / `x86_64`).
- Guest arch matches host arch for this spike (no cross-arch Linux on Apple Silicon).

## Guest image strategy

- **Kernel / initrd**: Alpine `latest-stable` `netboot/vmlinuz-virt` + `initramfs-virt` (downloaded on provision).
- **Root disk**: 4 GiB `ext4` on `vm/disks/root.raw`, built once by `scripts/build-guest-disk.sh` (Docker + `rust:bookworm` cross-builds `gptbot-guest-agent` for `*-unknown-linux-musl`).
- **Agent**: OpenRC service `gptbot-guest-agent` on boot, listens on vsock port **1024**.

## VM storage layout

Under app data (`~/Library/Application Support/com.drewsepeczi.gptbot/`):

```text
vm/
  config/vm.json
  disks/root.raw
  artifacts/vmlinuz-virt, initramfs-virt
  runtime/vmm.sock
  logs/vmm.log
```

## Entitlements

`src-tauri/entitlements.plist`:

- `com.apple.security.virtualization` = true

Bundled via `tauri.conf.json` → `bundle.macOS.entitlements`.

**Local dev:** unsigned `cargo run` binaries may need ad-hoc signing, e.g.:

```bash
codesign -s - --entitlements src-tauri/entitlements.plist --force \
  macos/gptbot-vmm/.build/release/gptbot-vmm
```

## Provisioning steps

1. Open Elsewhere → header **VM** → **Provision** (or `cargo run --example vm_acceptance -- <app_data> provision`).
2. Downloads Alpine netboot artifacts and runs `build-guest-disk.sh` (requires Docker).
3. Writes `vm/config/vm.json`.

## Lifecycle states

`notCreated` → `stopped` → `starting` → `running` → `stopping` → `stopped` / `error`

Swift VMM reports real `VZVirtualMachine` state; start waits for guest `ping` (no arbitrary sleep).

## Known limitations

- **Docker required** for first-time disk build.
- **No desktop / Chromium** — headless Alpine only.
- **Single VM** (`default` id).
- **Virtio socket I/O** must stay request/response sized (avoid huge streaming payloads).
- **Entitlement**: VM start fails with a clear error if virtualization entitlement is missing.
- Guest agent uses **one connection per request** on the host side (fine for spike RPC).

## Acceptance test

Script: `scripts/vm-acceptance-test.sh` (wraps `cargo run --example vm_acceptance`).

### Results (fill after running on hardware)

| Step | Status | Notes |
|------|--------|-------|
| Provision (artifacts + disk) | **Verified** | Alpine netboot download + Docker disk build (`root.raw` ~4 GiB) |
| VMM control plane | **Verified** | `status` over Unix socket; queue-correct `VZVirtualMachine.start` |
| VM boot + guest agent | **In progress** | Requires `codesign --generate-entitlement-der`; Alpine kernel/disk tuning ongoing |
| Persistence proof | _pending_ | Blocked on successful guest boot |

Run locally:

```bash
./scripts/vm-acceptance-test.sh
```

Record output in this table after a successful run.
