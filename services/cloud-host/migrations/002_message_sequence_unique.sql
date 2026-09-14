CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_conversation_sequence_unique
    ON messages(conversation_id, sequence);
