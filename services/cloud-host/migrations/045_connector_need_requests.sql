ALTER TABLE connector_oauth_states
  ADD COLUMN return_to text;

CREATE TABLE connector_need_requests (
  id text PRIMARY KEY,
  owner_id text NOT NULL,
  run_id text NOT NULL,
  bot_id text NOT NULL,
  provider text NOT NULL,
  tool_name text NOT NULL,
  reason_kind text NOT NULL,
  repo_owner text,
  repo_name text,
  status text NOT NULL,
  resolution text,
  created_at timestamptz NOT NULL,
  resolved_at timestamptz,
  CONSTRAINT connector_need_provider_github CHECK (provider = 'github'),
  CONSTRAINT connector_need_reason_kind CHECK (
    reason_kind IN (
      'disconnected',
      'reconnect_required',
      'unauthorized_repo',
      'empty_authorization'
    )
  ),
  CONSTRAINT connector_need_reason_repo CHECK (
    (
      reason_kind = 'unauthorized_repo'
      AND repo_owner IS NOT NULL
      AND repo_name IS NOT NULL
    )
    OR (
      reason_kind <> 'unauthorized_repo'
      AND repo_owner IS NULL
      AND repo_name IS NULL
    )
  ),
  CONSTRAINT connector_need_status_resolution CHECK (
    (status = 'pending' AND resolution IS NULL AND resolved_at IS NULL)
    OR (
      status = 'resolved'
      AND resolution IN (
        'connected',
        'repo_authorized',
        'dismissed',
        'expired',
        'cancelled'
      )
      AND resolved_at IS NOT NULL
    )
  )
);

CREATE UNIQUE INDEX connector_need_requests_pending_run_provider
  ON connector_need_requests (run_id, provider)
  WHERE status = 'pending';

CREATE INDEX connector_need_requests_owner_pending_idx
  ON connector_need_requests (owner_id, provider)
  WHERE status = 'pending';
