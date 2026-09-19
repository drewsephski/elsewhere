-- Deleting a Bot must not fail because it still appears in group history.
-- Direct conversations stay app-deleted (messages have no conversation CASCADE).
-- Group conversations remain; membership and thread state go away with the Bot.

ALTER TABLE conversation_participants
    DROP CONSTRAINT IF EXISTS conversation_participants_bot_id_fkey;
ALTER TABLE conversation_participants
    ADD CONSTRAINT conversation_participants_bot_id_fkey
        FOREIGN KEY (bot_id) REFERENCES bots(id) ON DELETE CASCADE;

ALTER TABLE conversation_bot_threads
    DROP CONSTRAINT IF EXISTS conversation_bot_threads_bot_id_fkey;
ALTER TABLE conversation_bot_threads
    ADD CONSTRAINT conversation_bot_threads_bot_id_fkey
        FOREIGN KEY (bot_id) REFERENCES bots(id) ON DELETE CASCADE;

ALTER TABLE messages
    DROP CONSTRAINT IF EXISTS messages_author_bot_id_fkey;
ALTER TABLE messages
    ADD CONSTRAINT messages_author_bot_id_fkey
        FOREIGN KEY (author_bot_id) REFERENCES bots(id) ON DELETE SET NULL;

ALTER TABLE group_message_recipients
    DROP CONSTRAINT IF EXISTS group_message_recipients_bot_id_fkey;
ALTER TABLE group_message_recipients
    ADD CONSTRAINT group_message_recipients_bot_id_fkey
        FOREIGN KEY (bot_id) REFERENCES bots(id) ON DELETE CASCADE;

ALTER TABLE run_subagents
    DROP CONSTRAINT IF EXISTS run_subagents_bot_id_fkey;
ALTER TABLE run_subagents
    ADD CONSTRAINT run_subagents_bot_id_fkey
        FOREIGN KEY (bot_id) REFERENCES bots(id) ON DELETE CASCADE;

ALTER TABLE bot_delegations
    DROP CONSTRAINT IF EXISTS bot_delegations_source_bot_id_fkey;
ALTER TABLE bot_delegations
    ADD CONSTRAINT bot_delegations_source_bot_id_fkey
        FOREIGN KEY (source_bot_id) REFERENCES bots(id) ON DELETE CASCADE;

ALTER TABLE bot_delegations
    DROP CONSTRAINT IF EXISTS bot_delegations_target_bot_id_fkey;
ALTER TABLE bot_delegations
    ADD CONSTRAINT bot_delegations_target_bot_id_fkey
        FOREIGN KEY (target_bot_id) REFERENCES bots(id) ON DELETE CASCADE;

ALTER TABLE bot_delegations
    DROP CONSTRAINT IF EXISTS bot_delegations_parent_delegation_id_fkey;
ALTER TABLE bot_delegations
    ADD CONSTRAINT bot_delegations_parent_delegation_id_fkey
        FOREIGN KEY (parent_delegation_id) REFERENCES bot_delegations(id) ON DELETE SET NULL;

ALTER TABLE bot_creations
    DROP CONSTRAINT IF EXISTS bot_creations_source_bot_id_fkey;
ALTER TABLE bot_creations
    ADD CONSTRAINT bot_creations_source_bot_id_fkey
        FOREIGN KEY (source_bot_id) REFERENCES bots(id) ON DELETE CASCADE;

ALTER TABLE bot_creations
    DROP CONSTRAINT IF EXISTS bot_creations_created_bot_id_fkey;
ALTER TABLE bot_creations
    ADD CONSTRAINT bot_creations_created_bot_id_fkey
        FOREIGN KEY (created_bot_id) REFERENCES bots(id) ON DELETE CASCADE;

ALTER TABLE channel_connections
    DROP CONSTRAINT IF EXISTS channel_connections_default_bot_id_fkey;
ALTER TABLE channel_connections
    ADD CONSTRAINT channel_connections_default_bot_id_fkey
        FOREIGN KEY (default_bot_id) REFERENCES bots(id) ON DELETE SET NULL;

ALTER TABLE channel_oauth_states
    DROP CONSTRAINT IF EXISTS channel_oauth_states_bot_id_fkey;
ALTER TABLE channel_oauth_states
    ADD CONSTRAINT channel_oauth_states_bot_id_fkey
        FOREIGN KEY (bot_id) REFERENCES bots(id) ON DELETE SET NULL;

ALTER TABLE channel_threads
    DROP CONSTRAINT IF EXISTS channel_threads_bot_id_fkey;
ALTER TABLE channel_threads
    ADD CONSTRAINT channel_threads_bot_id_fkey
        FOREIGN KEY (bot_id) REFERENCES bots(id) ON DELETE CASCADE;
