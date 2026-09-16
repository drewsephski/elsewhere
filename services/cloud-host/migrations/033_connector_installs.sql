-- Owner-installed MCP and OpenAPI integrations. Separate from GitHub OAuth rows.

CREATE TABLE connector_installs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('mcp', 'openapi')),
    display_name TEXT NOT NULL,
    endpoint_url TEXT NOT NULL,
    config JSONB NOT NULL DEFAULT '{}'::jsonb,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    status TEXT NOT NULL CHECK (status IN (
        'connected',
        'disabled',
        'error',
        'reconnect_required'
    )),
    last_discovery_error TEXT,
    last_discovered_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX connector_installs_owner_idx ON connector_installs (owner_id);
CREATE INDEX connector_installs_owner_enabled_idx ON connector_installs (owner_id, enabled);

CREATE TABLE connector_install_secrets (
    install_id UUID PRIMARY KEY REFERENCES connector_installs (id) ON DELETE CASCADE,
    ciphertext BYTEA NOT NULL,
    nonce BYTEA NOT NULL,
    key_version INT NOT NULL DEFAULT 1,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE connector_install_tools (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    install_id UUID NOT NULL REFERENCES connector_installs (id) ON DELETE CASCADE,
    remote_name TEXT NOT NULL,
    display_name TEXT NOT NULL,
    description TEXT NOT NULL,
    input_schema JSONB NOT NULL,
    read_only BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (install_id, remote_name)
);

CREATE INDEX connector_install_tools_install_idx ON connector_install_tools (install_id);

CREATE TABLE connector_mcp_oauth_sessions (
    state TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    install_id UUID REFERENCES connector_installs (id) ON DELETE CASCADE,
    code_verifier_nonce BYTEA NOT NULL,
    code_verifier_ciphertext BYTEA NOT NULL,
    pending_config JSONB NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX connector_mcp_oauth_sessions_owner_idx ON connector_mcp_oauth_sessions (owner_id);
CREATE INDEX connector_mcp_oauth_sessions_expires_idx ON connector_mcp_oauth_sessions (expires_at);
