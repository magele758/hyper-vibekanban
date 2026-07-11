-- Per-agent LLM credentials (NOT Electric-synced — secrets stay server-side)
CREATE TABLE agent_llm_settings (
    agent_id UUID PRIMARY KEY REFERENCES agents(id) ON DELETE CASCADE,
    api_key TEXT,
    base_url TEXT,
    model_name TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Sessions can belong to a board agent (NULL = project-level Copilot)
ALTER TABLE copilot_sessions
    ADD COLUMN agent_id UUID REFERENCES agents(id) ON DELETE CASCADE;

ALTER TABLE copilot_sessions
    ADD COLUMN external_agent_id TEXT;

CREATE INDEX idx_copilot_sessions_agent_id
    ON copilot_sessions (agent_id)
    WHERE agent_id IS NOT NULL;
