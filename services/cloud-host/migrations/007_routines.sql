CREATE TABLE routines (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    bot_id TEXT NOT NULL REFERENCES bots(id),
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    instructions TEXT NOT NULL CHECK (length(instructions) BETWEEN 1 AND 100000),
    interval_minutes INTEGER NOT NULL CHECK (interval_minutes BETWEEN 15 AND 43200),
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    next_run_at TIMESTAMPTZ NOT NULL,
    last_run_id TEXT REFERENCES agent_runs(id),
    last_error TEXT,
    acknowledged_run_id TEXT REFERENCES agent_runs(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_routines_due ON routines(next_run_at) WHERE enabled;
CREATE INDEX idx_routines_owner ON routines(owner_id, created_at);
ALTER TABLE work_queue ADD COLUMN routine_id TEXT REFERENCES routines(id);
CREATE INDEX idx_work_queue_routine ON work_queue(routine_id, created_at);
