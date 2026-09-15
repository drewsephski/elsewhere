CREATE INDEX IF NOT EXISTS agent_runs_assistant_message_id_idx
    ON agent_runs (assistant_message_id)
    WHERE assistant_message_id IS NOT NULL;
