-- Revision sessions: resume Elsewhere-managed open PRs and push follow-up commits.

ALTER TABLE github_coding_sessions
    ADD COLUMN IF NOT EXISTS session_mode TEXT NOT NULL DEFAULT 'initial',
    ADD COLUMN IF NOT EXISTS source_pr_number BIGINT,
    ADD COLUMN IF NOT EXISTS revision_baseline_commit_sha TEXT,
    ADD COLUMN IF NOT EXISTS revision_baseline_tree_sha TEXT;
