ALTER TABLE agent_runs ADD COLUMN IF NOT EXISTS archived_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_agent_runs_owner_active
    ON agent_runs (owner_id, created_at DESC)
    WHERE archived_at IS NULL;
