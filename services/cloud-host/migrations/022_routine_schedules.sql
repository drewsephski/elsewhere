-- Timezone-aware routine schedules, destinations, durable run history.

ALTER TABLE routines
    ADD COLUMN IF NOT EXISTS schedule_kind TEXT NOT NULL DEFAULT 'interval',
    ADD COLUMN IF NOT EXISTS schedule_expression TEXT,
    ADD COLUMN IF NOT EXISTS timezone TEXT NOT NULL DEFAULT 'UTC',
    ADD COLUMN IF NOT EXISTS destination_conversation_id TEXT REFERENCES conversations(id),
    ADD COLUMN IF NOT EXISTS last_success_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS last_failure_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS consecutive_failures INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS failure_policy TEXT NOT NULL DEFAULT 'pause_after_failure';

ALTER TABLE routines
    DROP CONSTRAINT IF EXISTS routines_schedule_kind_check;

ALTER TABLE routines
    ADD CONSTRAINT routines_schedule_kind_check
        CHECK (schedule_kind IN ('interval', 'daily', 'weekly', 'cron'));

ALTER TABLE routines
    DROP CONSTRAINT IF EXISTS routines_failure_policy_check;

ALTER TABLE routines
    ADD CONSTRAINT routines_failure_policy_check
        CHECK (failure_policy IN ('pause_after_failure', 'continue'));

UPDATE routines
SET schedule_kind = 'interval',
    schedule_expression = interval_minutes::TEXT,
    timezone = 'UTC'
WHERE schedule_expression IS NULL;

ALTER TABLE routines
    ALTER COLUMN schedule_expression SET NOT NULL;

CREATE TABLE routine_runs (
    id TEXT PRIMARY KEY,
    routine_id TEXT NOT NULL REFERENCES routines(id) ON DELETE CASCADE,
    owner_id TEXT NOT NULL,
    run_id TEXT REFERENCES agent_runs(id),
    scheduled_for TIMESTAMPTZ NOT NULL,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    status TEXT NOT NULL CHECK (
        status IN ('queued', 'running', 'completed', 'failed', 'cancelled', 'skipped')
    ),
    error_code TEXT,
    error_message TEXT,
    trigger_kind TEXT NOT NULL CHECK (trigger_kind IN ('scheduled', 'test', 'manual')),
    idempotency_key TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_routine_runs_scheduled_occurrence
    ON routine_runs (routine_id, scheduled_for, trigger_kind)
    WHERE trigger_kind = 'scheduled';

CREATE UNIQUE INDEX idx_routine_runs_idempotency
    ON routine_runs (routine_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;

CREATE INDEX idx_routine_runs_routine_created
    ON routine_runs (routine_id, created_at DESC);

CREATE INDEX idx_routine_runs_run
    ON routine_runs (run_id)
    WHERE run_id IS NOT NULL;
