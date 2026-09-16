-- Persistent Bot permission policies: owner defaults plus per-Bot overrides.
-- v1 keys policies by canonical tool/action. resource_scope is reserved for
-- future constrained scopes (repository, domain, directory) without replacing the model.

CREATE TABLE owner_permission_policies (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    action_key TEXT NOT NULL,
    decision TEXT NOT NULL CHECK (decision IN ('allow', 'ask', 'deny')),
    resource_scope JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT owner_permission_policies_owner_action UNIQUE (owner_id, action_key)
);

CREATE INDEX idx_owner_permission_policies_owner
    ON owner_permission_policies (owner_id);

CREATE TABLE bot_permission_policies (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    bot_id TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
    action_key TEXT NOT NULL,
    decision TEXT NOT NULL CHECK (decision IN ('allow', 'ask', 'deny')),
    resource_scope JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT bot_permission_policies_bot_action UNIQUE (bot_id, action_key)
);

CREATE INDEX idx_bot_permission_policies_owner
    ON bot_permission_policies (owner_id);

CREATE INDEX idx_bot_permission_policies_bot
    ON bot_permission_policies (bot_id);
