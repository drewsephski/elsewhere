-- Explicit result-finalization lifecycle for completed runs (artifact relay ordering).

ALTER TABLE agent_runs
    ADD COLUMN IF NOT EXISTS results_status TEXT,
    ADD COLUMN IF NOT EXISTS results_finalized_at TIMESTAMPTZ;

ALTER TABLE agent_runs
    DROP CONSTRAINT IF EXISTS agent_runs_results_status_check;

ALTER TABLE agent_runs
    ADD CONSTRAINT agent_runs_results_status_check
        CHECK (
            results_status IS NULL
            OR results_status IN ('pending', 'collecting', 'complete', 'partial', 'failed', 'skipped')
        );

-- Historical completed runs already released execution; treat as finalized.
UPDATE agent_runs
SET results_status = 'complete',
    results_finalized_at = COALESCE(results_finalized_at, execution_released_at, finished_at, updated_at)
WHERE status = 'completed'
  AND results_status IS NULL;

-- Terminal non-success runs do not wait on artifact collection.
UPDATE agent_runs
SET results_status = 'skipped',
    results_finalized_at = COALESCE(results_finalized_at, finished_at, updated_at)
WHERE status IN ('failed', 'cancelled', 'interrupted')
  AND results_status IS NULL;

CREATE INDEX IF NOT EXISTS idx_agent_runs_results_pending
    ON agent_runs (id)
    WHERE status = 'completed'
      AND results_status IN ('pending', 'collecting');
