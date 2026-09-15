CREATE TABLE human_intervention_requests (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    owner_id TEXT NOT NULL,
    computer_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    message TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX human_intervention_requests_run_id_idx ON human_intervention_requests (run_id);
CREATE INDEX human_intervention_requests_owner_status_idx ON human_intervention_requests (owner_id, status);

CREATE UNIQUE INDEX human_intervention_requests_one_pending_per_run
    ON human_intervention_requests (run_id)
    WHERE status = 'pending';
