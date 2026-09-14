-- Durable asynchronous bot-to-bot delegation (handoff).

CREATE TABLE bot_delegations (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    source_bot_id TEXT NOT NULL REFERENCES bots(id),
    target_bot_id TEXT NOT NULL REFERENCES bots(id),
    source_run_id TEXT NOT NULL REFERENCES agent_runs(id),
    source_conversation_id TEXT NOT NULL REFERENCES conversations(id),
    source_request_id TEXT NOT NULL,
    target_run_id TEXT REFERENCES agent_runs(id),
    target_conversation_id TEXT REFERENCES conversations(id),
    root_run_id TEXT NOT NULL REFERENCES agent_runs(id),
    parent_delegation_id TEXT REFERENCES bot_delegations(id),
    depth INT NOT NULL CHECK (depth >= 0 AND depth <= 32),
    instruction TEXT NOT NULL,
    context TEXT,
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'completed', 'failed', 'cancelled')),
    error_code TEXT,
    error_message TEXT,
    tool_invocation_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_bot_delegations_idempotency
    ON bot_delegations(source_run_id, tool_invocation_id);
CREATE INDEX idx_bot_delegations_source_run ON bot_delegations(source_run_id);
CREATE INDEX idx_bot_delegations_root_run ON bot_delegations(root_run_id);
CREATE INDEX idx_bot_delegations_target_run ON bot_delegations(target_run_id);
CREATE INDEX idx_bot_delegations_owner ON bot_delegations(owner_id);

ALTER TABLE work_queue
    ADD COLUMN delegation_id TEXT REFERENCES bot_delegations(id),
    ADD COLUMN provenance_kind TEXT;

ALTER TABLE messages
    ADD COLUMN provenance JSONB;
