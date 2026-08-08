-- Agents as workforce: reviewer relationships + executor transparency
-- See docs/agents-as-workforce.md

-- 1. Review as a configured organizational relationship, not a prompt convention.
ALTER TABLE agents
    ADD COLUMN IF NOT EXISTS reviewer_agent_id UUID
        REFERENCES agents(id) ON DELETE SET NULL;

-- An agent cannot review its own work.
ALTER TABLE agents
    DROP CONSTRAINT IF EXISTS agents_reviewer_not_self_check;

ALTER TABLE agents
    ADD CONSTRAINT agents_reviewer_not_self_check
    CHECK (reviewer_agent_id IS NULL OR reviewer_agent_id <> id);

COMMENT ON COLUMN agents.reviewer_agent_id IS
    'When set, a review task is enqueued for this agent after each of this agent''s tasks completes successfully.';

CREATE INDEX IF NOT EXISTS idx_agents_reviewer_agent_id
    ON agents(reviewer_agent_id)
    WHERE reviewer_agent_id IS NOT NULL;

-- 2. New trigger kind so review tasks are distinguishable and never chain.
ALTER TYPE agent_task_trigger ADD VALUE IF NOT EXISTS 'review';

-- 3. Executor transparency: record which coding agent actually ran when the
--    requested one was unavailable and the host fell back to a default.
ALTER TABLE agent_tasks
    ADD COLUMN IF NOT EXISTS executor_note TEXT;

COMMENT ON COLUMN agent_tasks.executor_note IS
    'Set when the requested executor was unavailable/invalid and the host fell back. Makes silent downgrades visible on the board.';

-- 4. Link a review task back to the task it reviews.
ALTER TABLE agent_tasks
    ADD COLUMN IF NOT EXISTS reviews_task_id UUID
        REFERENCES agent_tasks(id) ON DELETE SET NULL;

COMMENT ON COLUMN agent_tasks.reviews_task_id IS
    'For trigger = review: the agent_task whose work this task reviews.';

CREATE INDEX IF NOT EXISTS idx_agent_tasks_reviews_task_id
    ON agent_tasks(reviews_task_id)
    WHERE reviews_task_id IS NOT NULL;
