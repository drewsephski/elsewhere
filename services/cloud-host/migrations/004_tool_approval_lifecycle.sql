-- Phase 3C.2: durable human approval lifecycle

ALTER TABLE tool_approval_requests
    ADD COLUMN IF NOT EXISTS resolved_by TEXT,
    ADD COLUMN IF NOT EXISTS resolution_reason TEXT,
    ADD COLUMN IF NOT EXISTS expires_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

UPDATE tool_approval_requests
SET requested_at = created_at
WHERE requested_at IS NULL;

ALTER TABLE tool_approval_requests
    DROP CONSTRAINT IF EXISTS tool_approval_requests_status_check;

ALTER TABLE tool_approval_requests
    ADD CONSTRAINT tool_approval_requests_status_check
    CHECK (status IN ('pending', 'approved', 'denied', 'cancelled', 'expired'));

CREATE INDEX IF NOT EXISTS idx_tool_approval_pending_expires
    ON tool_approval_requests (status, expires_at)
    WHERE status = 'pending';
