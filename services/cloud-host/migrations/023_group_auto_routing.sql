-- Autonomous responder selection for unmentioned group messages.

ALTER TABLE group_message_sends
    ADD COLUMN IF NOT EXISTS routing_mode TEXT NOT NULL DEFAULT 'specific'
        CHECK (routing_mode IN ('auto', 'specific', 'everyone')),
    ADD COLUMN IF NOT EXISTS routing_status TEXT NOT NULL DEFAULT 'resolved'
        CHECK (routing_status IN ('pending', 'routing', 'resolved', 'no_response', 'failed', 'cancelled')),
    ADD COLUMN IF NOT EXISTS routing_attempts INT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS routing_error TEXT,
    ADD COLUMN IF NOT EXISTS router_model TEXT,
    ADD COLUMN IF NOT EXISTS routing_claim_token TEXT,
    ADD COLUMN IF NOT EXISTS routing_lease_until TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS routed_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS decision_code TEXT,
    ADD COLUMN IF NOT EXISTS routing_candidate_fingerprint TEXT;

ALTER TABLE group_message_recipients
    DROP CONSTRAINT IF EXISTS group_message_recipients_routing_kind_check;

ALTER TABLE group_message_recipients
    ADD CONSTRAINT group_message_recipients_routing_kind_check
        CHECK (routing_kind IN ('mention', 'everyone', 'auto'));

UPDATE group_message_sends
SET routing_mode = 'specific',
    routing_status = 'resolved',
    routed_at = COALESCE(routed_at, created_at)
WHERE routing_status = 'resolved' OR routing_status IS NOT NULL;

UPDATE group_message_sends s
SET routing_mode = 'everyone'
WHERE EXISTS (
    SELECT 1 FROM group_message_recipients r
    WHERE r.message_id = s.message_id AND r.routing_kind = 'everyone'
);

CREATE INDEX IF NOT EXISTS idx_group_message_sends_auto_pending
    ON group_message_sends (created_at ASC)
    WHERE routing_mode = 'auto'
      AND routing_status IN ('pending', 'routing');
