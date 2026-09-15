-- Product Skills registry (Open Agent Skills / SKILL.md)

CREATE TABLE skills (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    slug TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
    current_version INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (owner_id, slug)
);

CREATE INDEX idx_skills_owner ON skills (owner_id, updated_at DESC);

CREATE TABLE skill_versions (
    id TEXT PRIMARY KEY,
    skill_id TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
    owner_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    skill_md TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    parsed_name TEXT NOT NULL,
    parsed_description TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (skill_id, version)
);

CREATE INDEX idx_skill_versions_skill ON skill_versions (skill_id, version DESC);

CREATE TABLE skill_files (
    id TEXT PRIMARY KEY,
    skill_version_id TEXT NOT NULL REFERENCES skill_versions(id) ON DELETE CASCADE,
    relative_path TEXT NOT NULL,
    content TEXT NOT NULL,
    content_type TEXT,
    size INTEGER NOT NULL,
    sha256 TEXT NOT NULL,
    UNIQUE (skill_version_id, relative_path)
);

CREATE TABLE bot_skills (
    bot_id TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
    skill_id TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
    owner_id TEXT NOT NULL,
    pinned_version INTEGER,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (bot_id, skill_id)
);

CREATE INDEX idx_bot_skills_skill ON bot_skills (skill_id);

CREATE TABLE run_skills (
    run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    skill_id TEXT NOT NULL REFERENCES skills(id),
    skill_version_id TEXT NOT NULL REFERENCES skill_versions(id),
    invocation_kind TEXT NOT NULL CHECK (invocation_kind IN ('attached', 'explicit', 'routine')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (run_id, skill_id)
);

CREATE INDEX idx_run_skills_version ON run_skills (skill_version_id);

ALTER TABLE routines
    ADD COLUMN IF NOT EXISTS skill_id TEXT REFERENCES skills(id),
    ADD COLUMN IF NOT EXISTS pinned_skill_version INTEGER;

ALTER TABLE messages
    ADD COLUMN IF NOT EXISTS skill_invocation JSONB;
