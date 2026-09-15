-- Allow resume_status=running for source continuation lifecycle.

ALTER TABLE bot_delegations
    DROP CONSTRAINT IF EXISTS bot_delegations_resume_status_check;

ALTER TABLE bot_delegations
    ADD CONSTRAINT bot_delegations_resume_status_check
        CHECK (
            resume_status IS NULL
            OR resume_status IN ('pending', 'queued', 'running', 'completed', 'failed', 'skipped')
        );
