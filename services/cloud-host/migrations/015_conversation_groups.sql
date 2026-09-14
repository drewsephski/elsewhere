-- Group conversations, durable authorship, and per-bot provider thread state.

ALTER TABLE conversations
    ADD COLUMN IF NOT EXISTS conversation_type TEXT NOT NULL DEFAULT 'direct'
        CHECK (conversation_type IN ('direct', 'group')),
    ADD COLUMN IF NOT EXISTS name TEXT;

ALTER TABLE conversations
    ALTER COLUMN bot_id DROP NOT NULL;

CREATE TABLE IF NOT EXISTS conversation_participants (
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    bot_id TEXT NOT NULL REFERENCES bots(id),
    owner_id TEXT NOT NULL,
    joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    left_at TIMESTAMPTZ,
    ordinal INT NOT NULL DEFAULT 0,
    PRIMARY KEY (conversation_id, bot_id)
);

CREATE INDEX IF NOT EXISTS idx_conversation_participants_owner
    ON conversation_participants(owner_id);
CREATE INDEX IF NOT EXISTS idx_conversation_participants_bot_active
    ON conversation_participants(bot_id) WHERE left_at IS NULL;

ALTER TABLE messages
    ADD COLUMN IF NOT EXISTS author_kind TEXT NOT NULL DEFAULT 'human'
        CHECK (author_kind IN ('human', 'bot', 'system')),
    ADD COLUMN IF NOT EXISTS author_bot_id TEXT REFERENCES bots(id);

UPDATE messages m
SET author_kind = 'human'
WHERE m.role = 'user';

UPDATE messages m
SET author_kind = 'bot',
    author_bot_id = c.bot_id
FROM conversations c
WHERE m.conversation_id = c.id
  AND m.role = 'assistant'
  AND c.conversation_type = 'direct'
  AND c.bot_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS conversation_bot_threads (
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    bot_id TEXT NOT NULL REFERENCES bots(id),
    codex_thread_id TEXT,
    codex_compacted_through_turns BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (conversation_id, bot_id)
);

INSERT INTO conversation_bot_threads (conversation_id, bot_id, codex_thread_id, codex_compacted_through_turns, updated_at)
SELECT c.id, c.bot_id, c.codex_thread_id, COALESCE(c.codex_compacted_through_turns, 0), c.updated_at
FROM conversations c
WHERE c.conversation_type = 'direct'
  AND c.bot_id IS NOT NULL
ON CONFLICT (conversation_id, bot_id) DO NOTHING;

CREATE INDEX IF NOT EXISTS idx_conversations_owner_updated
    ON conversations (owner_id, updated_at DESC);
