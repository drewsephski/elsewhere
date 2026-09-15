-- Durable owner-scoped browser control lease (bot vs human takeover).

CREATE TABLE IF NOT EXISTS computer_control_leases (
    computer_id TEXT PRIMARY KEY REFERENCES sandboxes(id) ON DELETE CASCADE,
    owner_id TEXT NOT NULL,
    holder TEXT NOT NULL CHECK (holder IN ('bot', 'human')),
    lease_id TEXT NOT NULL,
    acquired_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    heartbeat_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_computer_control_leases_owner
    ON computer_control_leases(owner_id);
