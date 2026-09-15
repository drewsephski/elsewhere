-- Durable Chromium user-data mapping per computer (host volume holds bytes; this table holds opaque ids only).
CREATE TABLE browser_profiles (
    computer_id TEXT PRIMARY KEY REFERENCES sandboxes(id) ON DELETE CASCADE,
    owner_id TEXT NOT NULL,
    profile_id UUID NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_browser_profiles_owner ON browser_profiles(owner_id);
