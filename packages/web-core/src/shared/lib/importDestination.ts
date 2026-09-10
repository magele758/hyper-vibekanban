import type { ImportWebExportResult } from 'shared/types';

export type ImportDestination =
  | { kind: 'project'; projectId: string }
  | { kind: 'overview' };

/** Lite-only: full clients keep imported rows on `/imported`. */
export function destinationAfterWebImport(
  result: ImportWebExportResult
): ImportDestination {
  if (result.projects.length === 1) {
    return { kind: 'project', projectId: result.projects[0].id };
  }
  return { kind: 'overview' };
}
