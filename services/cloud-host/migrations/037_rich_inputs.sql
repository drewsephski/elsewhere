-- Rich Inputs v1: immutable owner-scoped attachments and same-run user questions.

CREATE TABLE attachments (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    bot_id TEXT REFERENCES bots (id) ON DELETE CASCADE,
    conversation_id TEXT REFERENCES conversations (id) ON DELETE CASCADE,
    original_name TEXT NOT NULL,
    safe_name TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size_bytes BIGINT NOT NULL,
    sha256 TEXT NOT NULL,
    content BYTEA NOT NULL,
    status TEXT NOT NULL DEFAULT 'staged'
        CHECK (status IN ('staged', 'attached', 'deleted')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    hidden_at TIMESTAMPTZ,
    CHECK (bot_id IS NOT NULL OR conversation_id IS NOT NULL)
);

CREATE INDEX attachments_owner_status_idx ON attachments (owner_id, status);
CREATE INDEX attachments_bot_idx ON attachments (bot_id);
CREATE INDEX attachments_staged_expiry_idx ON attachments (expires_at)
    WHERE status = 'staged';

CREATE TABLE message_attachments (
    message_id TEXT NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    attachment_id TEXT NOT NULL REFERENCES attachments (id) ON DELETE RESTRICT,
    ordinal INT NOT NULL,
    PRIMARY KEY (message_id, attachment_id),
    UNIQUE (message_id, ordinal)
);

CREATE INDEX message_attachments_attachment_idx ON message_attachments (attachment_id);

CREATE TABLE run_attachments (
    run_id TEXT NOT NULL REFERENCES agent_runs (id) ON DELETE CASCADE,
    attachment_id TEXT NOT NULL REFERENCES attachments (id) ON DELETE RESTRICT,
    ordinal INT NOT NULL,
    original_name TEXT NOT NULL,
    safe_name TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size_bytes BIGINT NOT NULL,
    sha256 TEXT NOT NULL,
    workspace_path TEXT NOT NULL,
    PRIMARY KEY (run_id, attachment_id),
    UNIQUE (run_id, ordinal)
);

CREATE INDEX run_attachments_attachment_idx ON run_attachments (attachment_id);

CREATE TABLE run_user_questions (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    run_id TEXT NOT NULL REFERENCES agent_runs (id) ON DELETE CASCADE,
    request_id TEXT NOT NULL,
    tool_invocation_id TEXT NOT NULL,
    question TEXT NOT NULL,
    options JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'answered', 'cancelled', 'interrupted')),
    selected_index INT,
    selected_option TEXT,
    requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    answered_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (run_id, tool_invocation_id)
);

CREATE UNIQUE INDEX run_user_questions_one_pending_per_run
    ON run_user_questions (run_id)
    WHERE status = 'pending';

CREATE INDEX run_user_questions_owner_status_idx ON run_user_questions (owner_id, status);
CREATE INDEX run_user_questions_run_idx ON run_user_questions (run_id);
