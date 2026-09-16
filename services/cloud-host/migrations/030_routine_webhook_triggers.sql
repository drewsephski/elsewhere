-- Event-triggered routines: explicit trigger mode plus hashed webhook tokens.

ALTER TABLE routines
    ADD COLUMN IF NOT EXISTS trigger_mode TEXT NOT NULL DEFAULT 'schedule';

ALTER TABLE routines
    DROP CONSTRAINT IF EXISTS routines_trigger_mode_check;

ALTER TABLE routines
    ADD CONSTRAINT routines_trigger_mode_check
        CHECK (trigger_mode IN ('schedule', 'webhook'));

UPDATE routines SET trigger_mode = 'schedule' WHERE trigger_mode IS NULL OR trigger_mode = '';

ALTER TABLE routine_runs
    DROP CONSTRAINT IF EXISTS routine_runs_trigger_kind_check;

ALTER TABLE routine_runs
    ADD CONSTRAINT routine_runs_trigger_kind_check
        CHECK (trigger_kind IN ('scheduled', 'test', 'manual', 'webhook'));

DROP INDEX IF EXISTS idx_routines_due;
CREATE INDEX idx_routines_due
    ON routines (next_run_at)
    WHERE enabled AND trigger_mode = 'schedule';

CREATE TABLE routine_webhook_triggers (
    id TEXT PRIMARY KEY,
    routine_id TEXT NOT NULL UNIQUE REFERENCES routines(id) ON DELETE CASCADE,
    owner_id TEXT NOT NULL,
    token_hash BYTEA NOT NULL UNIQUE,
    token_hint TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_triggered_at TIMESTAMPTZ
);

CREATE INDEX idx_routine_webhook_triggers_owner
    ON routine_webhook_triggers (owner_id, routine_id);
