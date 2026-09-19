ALTER TABLE github_coding_sessions
    ADD COLUMN IF NOT EXISTS certified_checks_json JSONB NOT NULL DEFAULT '[]'::jsonb,
    ADD COLUMN IF NOT EXISTS prepared_publish_json JSONB;
