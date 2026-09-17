-- Local Mac Companion pairing: possession-bound pairing sessions and
-- 1:1 durable node metadata for canonical sandboxes (provider = local_mac).
-- Raw pairing secrets and durable device credentials are never stored.
--
-- sandboxes.provider stays unconstrained TEXT, matching 001_init. A CHECK of
-- ('fly_sprite', 'local_mac') would reject existing test fixtures that use
-- provider = 'mock'. Application code writes only fly_sprite | local_mac
-- for real Computers.

CREATE TABLE local_mac_pairing_sessions (
    id TEXT PRIMARY KEY,
    installation_id TEXT NOT NULL,
    device_name TEXT NOT NULL,
    pairing_secret_hash BYTEA NOT NULL,
    user_code_hash BYTEA NOT NULL,
    owner_id TEXT,
    approved_at TIMESTAMPTZ,
    consumed_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_local_mac_pairing_sessions_installation
    ON local_mac_pairing_sessions (installation_id, created_at DESC);

CREATE INDEX idx_local_mac_pairing_sessions_expiry
    ON local_mac_pairing_sessions (expires_at)
    WHERE consumed_at IS NULL;

CREATE UNIQUE INDEX idx_local_mac_pairing_sessions_secret_hash
    ON local_mac_pairing_sessions (pairing_secret_hash);

CREATE TABLE local_mac_nodes (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    sandbox_id TEXT NOT NULL UNIQUE REFERENCES sandboxes(id),
    installation_id TEXT NOT NULL,
    device_name TEXT NOT NULL,
    credential_hash BYTEA NOT NULL,
    credential_hint TEXT NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_connected_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_local_mac_nodes_owner_installation
    ON local_mac_nodes (owner_id, installation_id);

CREATE INDEX idx_local_mac_nodes_owner
    ON local_mac_nodes (owner_id);
