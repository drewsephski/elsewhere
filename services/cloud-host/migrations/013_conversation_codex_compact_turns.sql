-- Track the last completed-turn milestone compacted for Codex (24, 48, 72, …).

ALTER TABLE conversations
    ADD COLUMN IF NOT EXISTS codex_compacted_through_turns BIGINT NOT NULL DEFAULT 0;
