-- Soft-delete for transcript messages removed from chat and work views.

ALTER TABLE messages
    ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_messages_conversation_active
    ON messages (conversation_id, sequence)
    WHERE deleted_at IS NULL;
