-- Board agent chat runtime: cursor (default) | pi | opencode
CREATE TYPE agent_chat_runtime AS ENUM ('cursor', 'pi', 'opencode');

ALTER TABLE agents
    ADD COLUMN chat_runtime agent_chat_runtime NOT NULL DEFAULT 'cursor';
