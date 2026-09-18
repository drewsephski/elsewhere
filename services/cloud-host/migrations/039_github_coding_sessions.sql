-- Durable GitHub coding session metadata (no secrets; workspace files stay on the computer).

CREATE TABLE IF NOT EXISTS github_coding_sessions (
    owner_id TEXT NOT NULL,
    run_id TEXT NOT NULL,
    request_id TEXT NOT NULL,
    repo_owner TEXT NOT NULL,
    repo_name TEXT NOT NULL,
    full_name TEXT NOT NULL,
    checkout_path TEXT NOT NULL,
    base_branch TEXT NOT NULL,
    opened_base_commit_sha TEXT NOT NULL,
    opened_base_tree_sha TEXT NOT NULL,
    working_branch TEXT NOT NULL,
    local_baseline_commit_sha TEXT NOT NULL,
    reviewed_fingerprint TEXT,
    approved_fingerprint TEXT,
    review_completed BOOLEAN NOT NULL DEFAULT FALSE,
    validations_json JSONB NOT NULL DEFAULT '[]'::jsonb,
    checks_passed BOOLEAN,
    explicit_no_checks BOOLEAN NOT NULL DEFAULT FALSE,
    publish_phase TEXT,
    publish_commit_sha TEXT,
    pr_number BIGINT,
    pr_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (owner_id, run_id)
);

CREATE INDEX IF NOT EXISTS idx_github_coding_sessions_request
    ON github_coding_sessions (request_id);
