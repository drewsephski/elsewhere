# GPT Bot Roadmap

## 1. Foundation — **Phase 1 (this repo)**

- [x] Tauri 2 + React shell
- [x] SQLite persistence (bots, conversations, messages)
- [x] macOS Keychain for OpenAI API key
- [x] OpenAI model list + streaming chat
- [x] ModelProvider abstraction (TS) + Rust provider implementation
- [x] Core tests (Rust DB/stream, TS stream assembler)

## 2. Local Computer

- [ ] Virtualization.framework Linux guest
- [ ] Persistent agent VM disk
- [ ] Native bridge to guest

## 3. Agent Runtime

- [ ] Browser automation
- [ ] Terminal / filesystem tools
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
