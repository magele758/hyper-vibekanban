-- Squad editable pipeline/DAG + work target (Issue / Path / both)
-- + optional Autopilot → Squad targeting

ALTER TABLE squads
    ADD COLUMN IF NOT EXISTS pipeline JSONB NOT NULL DEFAULT '{"nodes":[],"edges":[]}'::jsonb,
    ADD COLUMN IF NOT EXISTS target_type TEXT NOT NULL DEFAULT 'path',
    ADD COLUMN IF NOT EXISTS issue_id UUID REFERENCES issues(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS working_directory TEXT;

-- Keep target_type constrained to known values
ALTER TABLE squads DROP CONSTRAINT IF EXISTS squads_target_type_check;
ALTER TABLE squads
    ADD CONSTRAINT squads_target_type_check
    CHECK (target_type IN ('issue', 'path', 'issue_and_path'));

COMMENT ON COLUMN squads.pipeline IS
    'DAG: {nodes:[{id,agent_id?,role?,prompt?,label?}], edges:[{id,source,target}], loop_config?:{...}}';
COMMENT ON COLUMN squads.target_type IS
    'issue | path | issue_and_path — Issue=goal/context, Path=agent cwd';
COMMENT ON COLUMN squads.issue_id IS
    'Default target Issue when target_type includes issue';
COMMENT ON COLUMN squads.working_directory IS
    'Default local codebase/workdir when target_type includes path';

CREATE INDEX IF NOT EXISTS idx_squads_issue_id ON squads(issue_id)
    WHERE issue_id IS NOT NULL;

ALTER TABLE autopilots
    ADD COLUMN IF NOT EXISTS squad_id UUID REFERENCES squads(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_autopilots_squad_id ON autopilots(squad_id)
    WHERE squad_id IS NOT NULL;
