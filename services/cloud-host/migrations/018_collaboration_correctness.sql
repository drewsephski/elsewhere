-- Group context boundaries, conversation-scoped idempotency, delegation return-to-source.

ALTER TABLE group_message_sends
    ADD COLUMN IF NOT EXISTS request_fingerprint TEXT NOT NULL DEFAULT '';

ALTER TABLE group_message_sends
    DROP CONSTRAINT IF EXISTS group_message_sends_owner_id_idempotency_key_key;

CREATE UNIQUE INDEX IF NOT EXISTS idx_group_message_sends_owner_conv_key
    ON group_message_sends (owner_id, conversation_id, idempotency_key);

ALTER TABLE agent_runs
    ADD COLUMN IF NOT EXISTS group_context_through_sequence BIGINT;

ALTER TABLE bot_delegations
    ADD COLUMN IF NOT EXISTS return_policy TEXT NOT NULL DEFAULT 'none'
        CHECK (return_policy IN ('none', 'resume_source'));

ALTER TABLE bot_delegations
    ADD COLUMN IF NOT EXISTS source_resume_run_id TEXT REFERENCES agent_runs(id);

ALTER TABLE bot_delegations
    ADD COLUMN IF NOT EXISTS resume_status TEXT
        CHECK (
            resume_status IS NULL
            OR resume_status IN ('pending', 'queued', 'completed', 'failed', 'skipped')
        );

ALTER TABLE bot_delegations
    ADD COLUMN IF NOT EXISTS resume_error TEXT;

ALTER TABLE bot_delegations
    ADD COLUMN IF NOT EXISTS resume_created_at TIMESTAMPTZ;

CREATE UNIQUE INDEX IF NOT EXISTS idx_bot_delegations_one_resume_run
    ON bot_delegations (source_resume_run_id)
    WHERE source_resume_run_id IS NOT NULL;
