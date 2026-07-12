-- Working directory for Cursor SDK / local file ops (optional; empty = sidecar process.cwd())
ALTER TABLE agent_llm_settings
    ADD COLUMN IF NOT EXISTS working_directory TEXT;
