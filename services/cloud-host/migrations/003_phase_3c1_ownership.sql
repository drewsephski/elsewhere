-- Phase 3C.1: resource ownership, bot engine preference, tool approval records

ALTER TABLE bots ADD COLUMN IF NOT EXISTS owner_id TEXT;
ALTER TABLE bots ADD COLUMN IF NOT EXISTS engine_preference TEXT NOT NULL DEFAULT 'auto';

ALTER TABLE sandboxes ADD COLUMN IF NOT EXISTS owner_id TEXT;
ALTER TABLE sandboxes ADD COLUMN IF NOT EXISTS display_name TEXT NOT NULL DEFAULT '';

ALTER TABLE conversations ADD COLUMN IF NOT EXISTS owner_id TEXT;

ALTER TABLE agent_runs ADD COLUMN IF NOT EXISTS owner_id TEXT;

UPDATE bots SET owner_id = 'legacy-local' WHERE owner_id IS NULL;
UPDATE sandboxes SET owner_id = 'legacy-local' WHERE owner_id IS NULL;
UPDATE conversations SET owner_id = 'legacy-local' WHERE owner_id IS NULL;
UPDATE agent_runs SET owner_id = 'legacy-local' WHERE owner_id IS NULL;

ALTER TABLE bots ALTER COLUMN owner_id SET NOT NULL;
ALTER TABLE sandboxes ALTER COLUMN owner_id SET NOT NULL;
ALTER TABLE conversations ALTER COLUMN owner_id SET NOT NULL;
ALTER TABLE agent_runs ALTER COLUMN owner_id SET NOT NULL;

CREATE INDEX IF NOT EXISTS idx_bots_owner ON bots(owner_id);
CREATE INDEX IF NOT EXISTS idx_sandboxes_owner ON sandboxes(owner_id);
CREATE INDEX IF NOT EXISTS idx_conversations_owner ON conversations(owner_id);
CREATE INDEX IF NOT EXISTS idx_agent_runs_owner ON agent_runs(owner_id);
CREATE INDEX IF NOT EXISTS idx_agent_runs_owner_created ON agent_runs(owner_id, created_at DESC);

CREATE TABLE IF NOT EXISTS tool_approval_requests (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES agent_runs(id),
    owner_id TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    tool_kind TEXT NOT NULL,
    arguments_json JSONB NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_tool_approval_run ON tool_approval_requests(run_id);
CREATE INDEX IF NOT EXISTS idx_tool_approval_owner_status ON tool_approval_requests(owner_id, status);
