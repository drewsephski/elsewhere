-- Scoped permission policies for exact connected-app mutation tools.
-- Empty scope_key preserves existing unscoped owner/bot action uniqueness.

ALTER TABLE owner_permission_policies
    ADD COLUMN IF NOT EXISTS scope_key TEXT NOT NULL DEFAULT '';

ALTER TABLE bot_permission_policies
    ADD COLUMN IF NOT EXISTS scope_key TEXT NOT NULL DEFAULT '';

ALTER TABLE owner_permission_policies
    DROP CONSTRAINT IF EXISTS owner_permission_policies_owner_action;

ALTER TABLE owner_permission_policies
    ADD CONSTRAINT owner_permission_policies_owner_action_scope
    UNIQUE (owner_id, action_key, scope_key);

ALTER TABLE bot_permission_policies
    DROP CONSTRAINT IF EXISTS bot_permission_policies_bot_action;

ALTER TABLE bot_permission_policies
    ADD CONSTRAINT bot_permission_policies_bot_action_scope
    UNIQUE (bot_id, action_key, scope_key);
