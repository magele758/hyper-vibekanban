-- Agentic SDLC slice: sync squad_runs for board badges / live progress.
-- Table already has REPLICA IDENTITY FULL from workflow_spine migration.

SELECT electric_sync_table('public', 'squad_runs');
