-- Durable artifact handoff rows (bytes remain in work_results).

CREATE TABLE delegation_artifact_transfers (
    id TEXT PRIMARY KEY,
    delegation_id TEXT NOT NULL REFERENCES bot_delegations(id) ON DELETE CASCADE,
    source_result_id UUID NOT NULL REFERENCES work_results(id) ON DELETE CASCADE,
    source_run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    source_computer_id TEXT NOT NULL,
    destination_computer_id TEXT NOT NULL,
    destination_path TEXT NOT NULL,
    name TEXT NOT NULL,
    size BIGINT NOT NULL,
    sha256 TEXT,
    status TEXT NOT NULL,
    skip_reason TEXT,
    error_code TEXT,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    CONSTRAINT delegation_artifact_transfers_status_check
        CHECK (status IN ('pending', 'transferring', 'completed', 'failed', 'skipped')),
    CONSTRAINT delegation_artifact_transfers_unique
        UNIQUE (delegation_id, source_result_id)
);

CREATE INDEX idx_delegation_artifact_transfers_delegation
    ON delegation_artifact_transfers (delegation_id);

CREATE INDEX idx_delegation_artifact_transfers_pending
    ON delegation_artifact_transfers (status)
    WHERE status IN ('pending', 'transferring');
