CREATE TABLE bot_creations (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    source_bot_id TEXT NOT NULL REFERENCES bots(id),
    source_run_id TEXT NOT NULL REFERENCES agent_runs(id),
    root_run_id TEXT NOT NULL REFERENCES agent_runs(id),
    created_bot_id TEXT NOT NULL REFERENCES bots(id),
    tool_invocation_id TEXT NOT NULL,
    request_fingerprint TEXT NOT NULL,
    name TEXT NOT NULL,
    instructions TEXT NOT NULL,
    avatar_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (source_run_id, tool_invocation_id)
);

CREATE INDEX bot_creations_owner_idx ON bot_creations (owner_id);
CREATE INDEX bot_creations_root_run_idx ON bot_creations (root_run_id);
