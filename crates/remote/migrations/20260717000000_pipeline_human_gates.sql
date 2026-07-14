-- Feature Babysitter: human decision gates (merge approval, etc.)

CREATE TABLE IF NOT EXISTS pipeline_human_gates (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id          UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    issue_id            UUID NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    squad_id            UUID REFERENCES squads(id) ON DELETE SET NULL,
    gate_kind           TEXT NOT NULL DEFAULT 'merge_approval',
    local_workspace_id  UUID,
    question            TEXT NOT NULL DEFAULT '',
    status              TEXT NOT NULL DEFAULT 'pending',
    payload             JSONB NOT NULL DEFAULT '{}'::jsonb,
    decision_note       TEXT,
    decided_by          UUID REFERENCES users(id) ON DELETE SET NULL,
    decided_at          TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT pipeline_human_gates_status_check
        CHECK (status IN ('pending', 'approved', 'rejected', 'expired'))
);

CREATE INDEX IF NOT EXISTS pipeline_human_gates_issue_idx
    ON pipeline_human_gates (issue_id, created_at DESC);

CREATE INDEX IF NOT EXISTS pipeline_human_gates_pending_idx
    ON pipeline_human_gates (status)
    WHERE status = 'pending';

COMMENT ON TABLE pipeline_human_gates IS
    'Squad human_gate nodes: Inbox-driven approve/reject before merge or other actions';
