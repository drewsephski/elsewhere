ALTER TABLE connector_need_requests
  DROP CONSTRAINT connector_need_reason_kind;

ALTER TABLE connector_need_requests
  ADD CONSTRAINT connector_need_reason_kind CHECK (
    reason_kind IN (
      'disconnected',
      'reconnect_required',
      'unauthorized_repo',
      'empty_authorization',
      'host_unconfigured'
    )
  );
