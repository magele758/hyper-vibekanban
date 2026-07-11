-- Board Agents phases 2-3: squads, autopilots, inbox, webhooks, issue_subscribers

-- =============================================
-- Squads
-- =============================================

CREATE TABLE squads (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    leader_agent_id UUID REFERENCES agents(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (project_id, name)
);

CREATE INDEX idx_squads_project_id ON squads(project_id);

CREATE TABLE squad_members (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    squad_id UUID NOT NULL REFERENCES squads(id) ON DELETE CASCADE,
    agent_id UUID REFERENCES agents(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT squad_members_agent_or_user CHECK (
        (agent_id IS NOT NULL AND user_id IS NULL)
        OR (agent_id IS NULL AND user_id IS NOT NULL)
    )
);

CREATE UNIQUE INDEX squad_members_squad_agent_unique
    ON squad_members (squad_id, agent_id)
    WHERE agent_id IS NOT NULL;

CREATE UNIQUE INDEX squad_members_squad_user_unique
    ON squad_members (squad_id, user_id)
    WHERE user_id IS NOT NULL;

CREATE INDEX idx_squad_members_squad_id ON squad_members(squad_id);

-- =============================================
-- Extend issue_assignees with squad_id
-- =============================================

ALTER TABLE issue_assignees
    ADD COLUMN squad_id UUID REFERENCES squads(id) ON DELETE CASCADE;

ALTER TABLE issue_assignees
    DROP CONSTRAINT IF EXISTS issue_assignees_user_or_agent;

ALTER TABLE issue_assignees
    ADD CONSTRAINT issue_assignees_user_or_agent_or_squad CHECK (
        (user_id IS NOT NULL AND agent_id IS NULL AND squad_id IS NULL)
        OR (user_id IS NULL AND agent_id IS NOT NULL AND squad_id IS NULL)
        OR (user_id IS NULL AND agent_id IS NULL AND squad_id IS NOT NULL)
    );

CREATE UNIQUE INDEX issue_assignees_issue_squad_unique
    ON issue_assignees (issue_id, squad_id)
    WHERE squad_id IS NOT NULL;

CREATE INDEX idx_issue_assignees_squad_id ON issue_assignees(squad_id)
    WHERE squad_id IS NOT NULL;

-- =============================================
-- Extend agent_tasks with phase 2-3 fields
-- =============================================

ALTER TABLE agent_tasks
    ADD COLUMN force_fresh_session BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN resume_session_id UUID,
    ADD COLUMN squad_id UUID REFERENCES squads(id) ON DELETE SET NULL,
    ADD COLUMN is_leader_task BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN preferred_repo_id TEXT;

-- =============================================
-- Autopilots
-- =============================================

CREATE TYPE autopilot_execution_mode AS ENUM ('create_issue', 'run_only');
CREATE TYPE autopilot_concurrency_policy AS ENUM ('skip', 'queue');

CREATE TABLE autopilots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    agent_id UUID REFERENCES agents(id) ON DELETE SET NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    execution_mode autopilot_execution_mode NOT NULL DEFAULT 'create_issue',
    cron_expression TEXT NOT NULL DEFAULT '0 * * * *',
    timezone TEXT NOT NULL DEFAULT 'UTC',
    concurrency_policy autopilot_concurrency_policy NOT NULL DEFAULT 'skip',
    issue_title_template TEXT NOT NULL DEFAULT 'Autopilot run {{date}}',
    issue_description_template TEXT NOT NULL DEFAULT '',
    next_run_at TIMESTAMPTZ,
    last_run_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_autopilots_project_id ON autopilots(project_id);
CREATE INDEX idx_autopilots_next_run ON autopilots(next_run_at)
    WHERE enabled = TRUE AND next_run_at IS NOT NULL;

-- =============================================
-- Autopilot runs (audit log)
-- =============================================

CREATE TYPE autopilot_run_status AS ENUM (
    'queued', 'running', 'completed', 'failed', 'skipped'
);

CREATE TABLE autopilot_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    autopilot_id UUID NOT NULL REFERENCES autopilots(id) ON DELETE CASCADE,
    status autopilot_run_status NOT NULL DEFAULT 'queued',
    planned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    issue_id UUID REFERENCES issues(id) ON DELETE SET NULL,
    agent_task_id UUID REFERENCES agent_tasks(id) ON DELETE SET NULL,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_autopilot_runs_autopilot_id ON autopilot_runs(autopilot_id, created_at DESC);

-- =============================================
-- Inbox items
-- =============================================

CREATE TABLE inbox_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    recipient_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    project_id UUID REFERENCES projects(id) ON DELETE CASCADE,
    issue_id UUID REFERENCES issues(id) ON DELETE SET NULL,
    type TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL DEFAULT '',
    payload JSONB NOT NULL DEFAULT '{}',
    read_at TIMESTAMPTZ,
    archived_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_inbox_items_recipient ON inbox_items(recipient_user_id, created_at DESC);
CREATE INDEX idx_inbox_items_unread ON inbox_items(recipient_user_id)
    WHERE read_at IS NULL AND archived_at IS NULL;

-- =============================================
-- Issue subscribers
-- =============================================

CREATE TABLE issue_subscribers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    issue_id UUID NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    reason TEXT NOT NULL DEFAULT 'manual',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (issue_id, user_id)
);

CREATE INDEX idx_issue_subscribers_issue_id ON issue_subscribers(issue_id);
CREATE INDEX idx_issue_subscribers_user_id ON issue_subscribers(user_id);

-- =============================================
-- Webhook endpoints
-- =============================================

CREATE TABLE webhook_endpoints (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    autopilot_id UUID REFERENCES autopilots(id) ON DELETE SET NULL,
    token TEXT NOT NULL UNIQUE DEFAULT encode(gen_random_bytes(24), 'hex'),
    signing_secret TEXT,
    name TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_webhook_endpoints_project_id ON webhook_endpoints(project_id);
CREATE UNIQUE INDEX idx_webhook_endpoints_token ON webhook_endpoints(token);

-- =============================================
-- Webhook deliveries
-- =============================================

CREATE TABLE webhook_deliveries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    webhook_endpoint_id UUID NOT NULL REFERENCES webhook_endpoints(id) ON DELETE CASCADE,
    dedupe_key TEXT,
    status TEXT NOT NULL DEFAULT 'received',
    request_body TEXT NOT NULL DEFAULT '',
    response_summary TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_webhook_deliveries_endpoint_id ON webhook_deliveries(webhook_endpoint_id, created_at DESC);

-- =============================================
-- Electric sync for UI-facing tables
-- =============================================

SELECT electric_sync_table('public', 'squads');
SELECT electric_sync_table('public', 'squad_members');
SELECT electric_sync_table('public', 'autopilots');
SELECT electric_sync_table('public', 'inbox_items');

-- Retry budget for agent tasks (watcher requeues while attempt < max_attempts)
ALTER TABLE agent_tasks ALTER COLUMN max_attempts SET DEFAULT 3;
UPDATE agent_tasks SET max_attempts = 3 WHERE max_attempts < 3;
