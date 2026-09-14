-- Structured group @mention routing, idempotent group sends, and per-bot group context cursors.

CREATE TABLE IF NOT EXISTS group_message_sends (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    idempotency_key TEXT NOT NULL,
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (owner_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_group_message_sends_conversation
    ON group_message_sends (conversation_id, created_at DESC);

CREATE TABLE IF NOT EXISTS group_message_recipients (
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    bot_id TEXT NOT NULL REFERENCES bots(id),
    run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
    routing_kind TEXT NOT NULL CHECK (routing_kind IN ('mention', 'everyone')),
    status TEXT NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued', 'running', 'completed', 'failed', 'cancelled')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (message_id, bot_id)
);

CREATE INDEX IF NOT EXISTS idx_group_message_recipients_conversation
    ON group_message_recipients (conversation_id, message_id);

ALTER TABLE agent_runs
    ADD COLUMN IF NOT EXISTS source_message_id TEXT REFERENCES messages(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_agent_runs_source_message
    ON agent_runs (source_message_id) WHERE source_message_id IS NOT NULL;

ALTER TABLE conversation_bot_threads
    ADD COLUMN IF NOT EXISTS last_seen_group_sequence BIGINT NOT NULL DEFAULT 0;
