-- Lightweight per-Bot setup session. Independent of agent_runs / ask_user.
-- Owner-authored pinned context remains in bot_context; this table only stores
-- the optional post-create questionnaire until it is applied or dismissed.

CREATE TABLE bot_onboarding (
    bot_id TEXT PRIMARY KEY REFERENCES bots(id) ON DELETE CASCADE,
    owner_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN (
        'not_started', 'in_progress', 'ready_to_apply', 'completed', 'dismissed'
    )),
    questions_asked INTEGER NOT NULL DEFAULT 0
        CHECK (questions_asked >= 0 AND questions_asked <= 3),
    current_question JSONB,
    answers JSONB NOT NULL DEFAULT '[]'::jsonb,
    draft_configuration JSONB,
    last_error TEXT,
    revision BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX bot_onboarding_owner ON bot_onboarding (owner_id, status);
