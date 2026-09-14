-- Execution inputs are committed with admission, so closing a client cannot lose work.
CREATE TABLE work_queue (
    run_id TEXT PRIMARY KEY REFERENCES agent_runs(id),
    user_message TEXT NOT NULL,
    instructions TEXT NOT NULL,
    engine_preference TEXT NOT NULL CHECK (engine_preference IN ('auto', 'codex', 'responses')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_agent_runs_computer_active ON agent_runs(computer_id) WHERE status = 'running';
CREATE INDEX idx_agent_runs_queued ON agent_runs(created_at) WHERE status = 'queued';
ALTER TABLE agent_runs ADD COLUMN cancel_requested BOOLEAN NOT NULL DEFAULT FALSE;
