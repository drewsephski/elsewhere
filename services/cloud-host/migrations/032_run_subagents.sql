-- Ephemeral parent-run helpers (not Bots, conversations, computers, or queued agent_runs).
CREATE TABLE run_subagents (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    parent_run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    bot_id TEXT NOT NULL REFERENCES bots(id),
    tool_invocation_id TEXT NOT NULL,
    name TEXT NOT NULL,
    task TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('running', 'completed', 'failed', 'cancelled', 'interrupted')),
    result TEXT,
    error TEXT,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ,
    UNIQUE (parent_run_id, tool_invocation_id)
);

CREATE INDEX run_subagents_parent_status_idx ON run_subagents (parent_run_id, status);
CREATE INDEX run_subagents_owner_idx ON run_subagents (owner_id, started_at DESC);
