-- Distinguish how a workspace's working tree is materialized.
--   'worktree'  -> dedicated git worktree per repo (default, current behavior)
--   'in_place'  -> the coding agent runs directly in the repo's own working tree
ALTER TABLE workspaces ADD COLUMN kind TEXT NOT NULL DEFAULT 'worktree';
