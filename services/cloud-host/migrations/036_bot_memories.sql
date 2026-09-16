-- Durable per-Bot memories, immutable run snapshots, and async extraction jobs.
-- Existing bot_context remains the always-included owner-authored pinned context.

ALTER TABLE bots
    ADD COLUMN IF NOT EXISTS learn_from_conversations BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE agent_runs
    ADD COLUMN IF NOT EXISTS executed_engine TEXT
        CHECK (executed_engine IS NULL OR executed_engine IN ('codex', 'responses'));

CREATE TABLE bot_memories (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    bot_id TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN (
        'preference', 'fact', 'project', 'constraint', 'workflow', 'relationship', 'other'
    )),
    content TEXT NOT NULL CHECK (octet_length(content) > 0 AND octet_length(content) <= 2048),
    content_normalized TEXT NOT NULL CHECK (octet_length(content_normalized) > 0),
    search_terms TEXT[] NOT NULL DEFAULT '{}',
    importance SMALLINT NOT NULL DEFAULT 3 CHECK (importance BETWEEN 1 AND 5),
    confidence REAL NOT NULL DEFAULT 0.8 CHECK (confidence >= 0 AND confidence <= 1),
    source_kind TEXT NOT NULL CHECK (source_kind IN ('manual', 'explicit_tool', 'automatic')),
    source_run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
    source_message_id TEXT,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'superseded', 'archived')),
    superseded_by TEXT REFERENCES bot_memories(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_confirmed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ,
    use_count INTEGER NOT NULL DEFAULT 0 CHECK (use_count >= 0),
    search_vector tsvector NOT NULL DEFAULT ''::tsvector
);

CREATE OR REPLACE FUNCTION bot_memories_refresh_search_vector()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    NEW.search_vector :=
        setweight(to_tsvector('simple', coalesce(NEW.content, '')), 'A') ||
        setweight(to_tsvector('simple', coalesce(array_to_string(NEW.search_terms, ' '), '')), 'B');
    RETURN NEW;
END
$$;

CREATE TRIGGER bot_memories_search_vector
BEFORE INSERT OR UPDATE OF content, search_terms ON bot_memories
FOR EACH ROW
EXECUTE FUNCTION bot_memories_refresh_search_vector();

CREATE INDEX bot_memories_bot_status ON bot_memories (owner_id, bot_id, status, last_confirmed_at DESC);
CREATE INDEX bot_memories_bot_kind ON bot_memories (owner_id, bot_id, kind) WHERE status = 'active';
CREATE INDEX bot_memories_search ON bot_memories USING GIN (search_vector);
CREATE UNIQUE INDEX bot_memories_active_normalized
    ON bot_memories (owner_id, bot_id, content_normalized)
    WHERE status = 'active';

CREATE TABLE run_memories (
    run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    memory_id TEXT NOT NULL,
    content TEXT NOT NULL,
    kind TEXT NOT NULL,
    rank INTEGER NOT NULL,
    score DOUBLE PRECISION,
    PRIMARY KEY (run_id, memory_id)
);

CREATE INDEX run_memories_run ON run_memories (run_id, rank);

CREATE TABLE memory_extraction_jobs (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    bot_id TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
    source_run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'completed', 'failed')),
    engine_kind TEXT NOT NULL CHECK (engine_kind IN ('codex', 'responses')),
    attempt_count INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 3,
    last_error_code TEXT,
    last_error TEXT,
    claimed_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (source_run_id)
);

CREATE INDEX memory_extraction_jobs_claim
    ON memory_extraction_jobs (status, created_at)
    WHERE status IN ('queued', 'running');
