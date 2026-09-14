-- Durable chat thread per bot: Codex app-server thread id for turn continuity.

ALTER TABLE conversations
    ADD COLUMN IF NOT EXISTS codex_thread_id TEXT;

CREATE INDEX IF NOT EXISTS idx_conversations_bot_updated
    ON conversations (bot_id, updated_at DESC);
