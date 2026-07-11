-- Agents as first-class assignees + task queue + board copilot sessions

CREATE TYPE agent_status AS ENUM ('idle', 'working', 'offline', 'error');
CREATE TYPE agent_task_status AS ENUM (
    'queued',
    'dispatched',
    'running',
    'completed',
    'failed',
    'cancelled'
);
CREATE TYPE agent_task_trigger AS ENUM (
    'assign',
    'mention',
    'manual',
    'copilot',
    'autopilot'
);

CREATE TABLE agents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    instructions TEXT NOT NULL DEFAULT '',
    default_executor TEXT,
    max_concurrent_tasks INTEGER NOT NULL DEFAULT 1
        CHECK (max_concurrent_tasks > 0),
    status agent_status NOT NULL DEFAULT 'idle',
    created_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (project_id, name)
);

CREATE INDEX idx_agents_project_id ON agents(project_id);

-- Polymorphic assignees: user XOR agent
ALTER TABLE issue_assignees
    ALTER COLUMN user_id DROP NOT NULL;

ALTER TABLE issue_assignees
    ADD COLUMN agent_id UUID REFERENCES agents(id) ON DELETE CASCADE;

ALTER TABLE issue_assignees
    DROP CONSTRAINT IF EXISTS issue_assignees_issue_id_user_id_key;

ALTER TABLE issue_assignees
    ADD CONSTRAINT issue_assignees_user_or_agent CHECK (
        (user_id IS NOT NULL AND agent_id IS NULL)
        OR (user_id IS NULL AND agent_id IS NOT NULL)
    );

CREATE UNIQUE INDEX issue_assignees_issue_user_unique
    ON issue_assignees (issue_id, user_id)
    WHERE user_id IS NOT NULL;

CREATE UNIQUE INDEX issue_assignees_issue_agent_unique
    ON issue_assignees (issue_id, agent_id)
    WHERE agent_id IS NOT NULL;

CREATE INDEX idx_issue_assignees_agent_id ON issue_assignees(agent_id)
    WHERE agent_id IS NOT NULL;

CREATE TABLE agent_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    issue_id UUID NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    status agent_task_status NOT NULL DEFAULT 'queued',
    trigger agent_task_trigger NOT NULL DEFAULT 'assign',
    priority INTEGER NOT NULL DEFAULT 0,
    attempt INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 1,
    failure_reason TEXT,
    local_workspace_id UUID,
    local_session_id UUID,
    claimed_by_host TEXT,
    claimed_at TIMESTAMPTZ,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_agent_tasks_queued
    ON agent_tasks (status, created_at)
    WHERE status = 'queued';

CREATE INDEX idx_agent_tasks_agent_id ON agent_tasks(agent_id);
CREATE INDEX idx_agent_tasks_issue_id ON agent_tasks(issue_id);

-- At most one active task per (agent, issue)
CREATE UNIQUE INDEX agent_tasks_active_unique
    ON agent_tasks (agent_id, issue_id)
    WHERE status IN ('queued', 'dispatched', 'running');

CREATE TABLE copilot_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    issue_id UUID REFERENCES issues(id) ON DELETE SET NULL,
    created_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    title TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_copilot_sessions_project_id ON copilot_sessions(project_id);

CREATE TABLE copilot_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES copilot_sessions(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'system', 'tool')),
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_copilot_messages_session_id ON copilot_messages(session_id, created_at);

-- Electric sync for new / changed tables
SELECT electric_sync_table('public', 'agents');
SELECT electric_sync_table('public', 'agent_tasks');
SELECT electric_sync_table('public', 'copilot_sessions');
SELECT electric_sync_table('public', 'copilot_messages');
