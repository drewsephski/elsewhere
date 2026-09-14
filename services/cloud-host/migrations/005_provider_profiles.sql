-- References only. Codex manages its own credentials on the trusted runner volume.
CREATE TABLE provider_profiles (
    owner_id TEXT PRIMARY KEY,
    profile_id UUID NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
