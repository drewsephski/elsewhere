-- Provider-neutral messaging channels (Slack v1) and conversation origin isolation.

ALTER TABLE conversations
    ADD COLUMN origin_kind TEXT NOT NULL DEFAULT 'web'
        CHECK (origin_kind IN ('web', 'channel'));

UPDATE conversations SET origin_kind = 'web' WHERE origin_kind IS NULL;

CREATE INDEX idx_conversations_primary_web_direct
    ON conversations (bot_id, owner_id, updated_at DESC)
    WHERE conversation_type = 'direct' AND origin_kind = 'web';

ALTER TABLE agent_runs
    ADD COLUMN origin_kind TEXT NOT NULL DEFAULT 'web'
        CHECK (origin_kind IN ('web', 'channel')),
    ADD COLUMN origin_provider TEXT;

UPDATE agent_runs SET origin_kind = 'web' WHERE origin_kind IS NULL;

CREATE TABLE channel_connections (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    provider TEXT NOT NULL CHECK (provider IN ('slack')),
    status TEXT NOT NULL CHECK (status IN ('connected', 'disconnected')),
    external_workspace_id TEXT,
    workspace_name TEXT,
    installer_external_user_id TEXT,
    bot_user_id TEXT,
    default_bot_id TEXT REFERENCES bots(id),
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_channel_connections_one_connected_per_owner_provider
    ON channel_connections (owner_id, provider)
    WHERE status = 'connected';

CREATE INDEX idx_channel_connections_workspace
    ON channel_connections (provider, external_workspace_id)
    WHERE status = 'connected';

CREATE TABLE channel_connection_secrets (
    connection_id TEXT PRIMARY KEY REFERENCES channel_connections(id) ON DELETE CASCADE,
    ciphertext BYTEA NOT NULL,
    nonce BYTEA NOT NULL,
    key_version INT NOT NULL DEFAULT 1,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE channel_oauth_states (
    state TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    bot_id TEXT REFERENCES bots(id),
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_channel_oauth_states_expiry ON channel_oauth_states (expires_at);

CREATE TABLE channel_threads (
    id TEXT PRIMARY KEY,
    connection_id TEXT NOT NULL REFERENCES channel_connections(id) ON DELETE CASCADE,
    owner_id TEXT NOT NULL,
    bot_id TEXT NOT NULL REFERENCES bots(id),
    provider TEXT NOT NULL,
    external_channel_id TEXT NOT NULL,
    external_thread_id TEXT NOT NULL,
    conversation_id TEXT NOT NULL REFERENCES conversations(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_activity_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (connection_id, external_channel_id, external_thread_id)
);

CREATE UNIQUE INDEX idx_channel_threads_conversation ON channel_threads (conversation_id);
CREATE INDEX idx_channel_threads_owner ON channel_threads (owner_id);

CREATE TABLE channel_events (
    id TEXT PRIMARY KEY,
    connection_id TEXT REFERENCES channel_connections(id) ON DELETE SET NULL,
    owner_id TEXT,
    provider TEXT NOT NULL,
    external_event_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    event_type TEXT,
    status TEXT NOT NULL CHECK (status IN ('received', 'admitted', 'ignored', 'rejected')),
    ignore_reason TEXT,
    run_id TEXT REFERENCES agent_runs(id),
    payload_json JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (provider, external_event_id)
);

CREATE INDEX idx_channel_events_connection_created
    ON channel_events (connection_id, created_at DESC);

CREATE TABLE channel_deliveries (
    id TEXT PRIMARY KEY,
    connection_id TEXT NOT NULL REFERENCES channel_connections(id) ON DELETE CASCADE,
    thread_id TEXT REFERENCES channel_threads(id) ON DELETE SET NULL,
    owner_id TEXT NOT NULL,
    source_run_id TEXT REFERENCES agent_runs(id),
    source_message_id TEXT,
    kind TEXT NOT NULL CHECK (kind IN ('assistant_reply', 'owner_attention')),
    chunk_index INT NOT NULL DEFAULT 0 CHECK (chunk_index >= 0 AND chunk_index < 8),
    body TEXT NOT NULL CHECK (char_length(body) <= 4000),
    status TEXT NOT NULL CHECK (status IN ('queued', 'sending', 'sent', 'retryable', 'failed')),
    attempts INT NOT NULL DEFAULT 0,
    provider_message_id TEXT,
    last_error TEXT,
    next_attempt_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    sent_at TIMESTAMPTZ,
    UNIQUE (source_run_id, kind, chunk_index)
);

CREATE INDEX idx_channel_deliveries_due
    ON channel_deliveries (status, next_attempt_at, created_at)
    WHERE status IN ('queued', 'retryable', 'sending');
