CREATE TABLE bot_context (
    bot_id TEXT PRIMARY KEY REFERENCES bots(id) ON DELETE CASCADE,
    content TEXT NOT NULL CHECK (octet_length(content) <= 16000),
    revision BIGINT NOT NULL DEFAULT 1,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- A computer remains occupied while completed deliverables are being captured.
ALTER TABLE agent_runs ADD COLUMN execution_released_at TIMESTAMPTZ;
UPDATE agent_runs SET execution_released_at = NOW() WHERE status NOT IN ('running', 'queued');
CREATE TABLE work_results (
    id UUID PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('summary', 'file')),
    content BYTEA NOT NULL CHECK (octet_length(content) <= 1048576),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (run_id, kind, name)
);
CREATE INDEX work_results_run ON work_results(run_id);
ALTER TABLE agent_runs ADD COLUMN results_note TEXT;
