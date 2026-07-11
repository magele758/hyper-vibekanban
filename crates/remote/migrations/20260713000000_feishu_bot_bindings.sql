-- Feishu (Lark) bot bindings for board agents

ALTER TYPE agent_task_trigger ADD VALUE IF NOT EXISTS 'feishu';

CREATE TABLE feishu_bot_bindings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    name TEXT NOT NULL DEFAULT '飞书机器人',
    app_id TEXT NOT NULL,
    app_secret TEXT NOT NULL,
    encrypt_key TEXT,
    verification_token TEXT,
    callback_token TEXT NOT NULL UNIQUE DEFAULT encode(gen_random_bytes(24), 'hex'),
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    reply_on_complete BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (project_id, app_id)
);

CREATE INDEX idx_feishu_bot_bindings_project_id ON feishu_bot_bindings(project_id);
CREATE INDEX idx_feishu_bot_bindings_agent_id ON feishu_bot_bindings(agent_id);
CREATE UNIQUE INDEX idx_feishu_bot_bindings_callback_token ON feishu_bot_bindings(callback_token);

-- Track inbound Feishu messages so we can reply when the agent task finishes.
CREATE TABLE feishu_inbound_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    binding_id UUID NOT NULL REFERENCES feishu_bot_bindings(id) ON DELETE CASCADE,
    message_id TEXT NOT NULL,
    chat_id TEXT NOT NULL,
    open_id TEXT,
    text_content TEXT NOT NULL DEFAULT '',
    agent_task_id UUID REFERENCES agent_tasks(id) ON DELETE SET NULL,
    issue_id UUID REFERENCES issues(id) ON DELETE SET NULL,
    reply_status TEXT NOT NULL DEFAULT 'pending',
    reply_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (binding_id, message_id)
);

CREATE INDEX idx_feishu_inbound_messages_task_id
    ON feishu_inbound_messages(agent_task_id)
    WHERE agent_task_id IS NOT NULL;
