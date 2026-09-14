# Elsewhere Roadmap

## 1. Foundation — **Phase 1 (this repo)**

- [x] Tauri 2 + React shell
- [x] SQLite persistence (bots, conversations, messages)
- [x] macOS Keychain for OpenAI API key
- [x] OpenAI model list + streaming chat
- [x] ModelProvider abstraction (TS) + Rust provider implementation
- [x] Core tests (Rust DB/stream, TS stream assembler)

## 2. Local Computer

- [x] Virtualization.framework Linux guest (Phase 2A spike — Alpine + Swift `gptbot-vmm`)
- [x] Persistent agent VM disk (`vm/disks/root.raw`, provision once)
- [x] Native bridge to guest (Virtio socket + `gptbot-guest-agent` JSON RPC)
- [ ] Production Agent Computer UI, desktop viewer, Chromium

## 3. Agent Runtime

- [x] Portable `agent-core` (Luna Responses tool loop, `AgentComputer`, `RunStore`, `EventSink`)
- [x] Terminal / filesystem tools via guest RPC (`workspace_*` tools + `LocalMacComputer`)
- [x] `run_events` table (dual-write with structured `messages` during migration)
- [ ] Browser automation
- [ ] Approvals and activity timeline

## 4. Persistence / Memory

- [ ] Long-term memory
- [ ] Semantic search over local files

## 5. Skills / Routines

- [ ] Skill packages
- [ ] Scheduled routines / background execution

## 6. Multi-Agent

- [ ] Bot groups
- [ ] Handoffs and delegation

## 7. Polish

- [ ] Updater
- [ ] Cloud sync (if ever)
- [ ] Production hardening
