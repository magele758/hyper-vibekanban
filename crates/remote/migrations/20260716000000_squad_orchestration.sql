-- Squad orchestration: step prompts on agent tasks + allow parallel squad tasks

ALTER TABLE agent_tasks
    ADD COLUMN IF NOT EXISTS execution_prompt TEXT;

COMMENT ON COLUMN agent_tasks.execution_prompt IS
    'Optional per-step prompt injected by squad pipeline (role/prompt/handoff)';

-- Non-squad tasks keep one-active-per-(agent,issue); squad pipeline may enqueue
-- serial/parallel steps for the same agent+issue.
DROP INDEX IF EXISTS agent_tasks_active_unique;
CREATE UNIQUE INDEX agent_tasks_active_unique
    ON agent_tasks (agent_id, issue_id)
    WHERE status IN ('queued', 'dispatched', 'running')
      AND squad_id IS NULL;
