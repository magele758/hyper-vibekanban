-- AI workflow spine Batch 1: on_assign, squad_runs, approval resume

ALTER TABLE squads
    ADD COLUMN IF NOT EXISTS on_assign TEXT NOT NULL DEFAULT 'leader_only';

ALTER TABLE squads
    DROP CONSTRAINT IF EXISTS squads_on_assign_check;

ALTER TABLE squads
    ADD CONSTRAINT squads_on_assign_check
    CHECK (on_assign IN ('leader_only', 'full_pipeline'));

COMMENT ON COLUMN squads.on_assign IS
    'leader_only: assign enqueues leader only (default). full_pipeline: assign starts squad pipeline.';

CREATE TABLE IF NOT EXISTS squad_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    squad_id UUID NOT NULL REFERENCES squads(id) ON DELETE CASCADE,
    issue_id UUID NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'running',
    start_from_node_id TEXT,
    pause_node_id TEXT,
    resume_node_id TEXT,
    approval_kind TEXT,
    approval_prompt TEXT,
    working_directory TEXT,
    ordered_node_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
    agent_task_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
    error_message TEXT,
    created_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT squad_runs_status_check CHECK (
        status IN (
            'queued',
            'running',
            'waiting_approval',
            'completed',
            'failed',
            'cancelled'
        )
    )
);

CREATE INDEX IF NOT EXISTS idx_squad_runs_issue_id ON squad_runs(issue_id);
CREATE INDEX IF NOT EXISTS idx_squad_runs_squad_id ON squad_runs(squad_id);
CREATE INDEX IF NOT EXISTS idx_squad_runs_status ON squad_runs(status)
    WHERE status IN ('running', 'waiting_approval', 'queued');

CREATE TABLE IF NOT EXISTS squad_run_nodes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES squad_runs(id) ON DELETE CASCADE,
    node_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    agent_task_id UUID REFERENCES agent_tasks(id) ON DELETE SET NULL,
    output_summary TEXT,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (run_id, node_id),
    CONSTRAINT squad_run_nodes_status_check CHECK (
        status IN (
            'pending',
            'running',
            'completed',
            'failed',
            'skipped',
            'waiting_approval'
        )
    )
);

CREATE INDEX IF NOT EXISTS idx_squad_run_nodes_run_id ON squad_run_nodes(run_id);

ALTER TABLE squad_runs REPLICA IDENTITY FULL;
ALTER TABLE squad_run_nodes REPLICA IDENTITY FULL;
