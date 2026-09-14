CREATE INDEX agent_runs_bot_recency ON agent_runs(bot_id, created_at DESC);
CREATE INDEX agent_runs_unreleased_computer ON agent_runs(computer_id) WHERE started_at IS NOT NULL AND execution_released_at IS NULL;
