import { describe, expect, it } from 'vitest';
import type { ImportWebExportResult } from 'shared/types';
import { destinationAfterWebImport } from './importDestination';

function result(
  projects: ImportWebExportResult['projects']
): ImportWebExportResult {
  return {
    projects_created: projects.filter((project) => !project.reused).length,
    projects_reused: projects.filter((project) => project.reused).length,
    issues_created: 0,
    issues_skipped: 0,
    projects,
  };
}

describe('destinationAfterWebImport', () => {
  it('opens the sole imported project', () => {
    expect(
      destinationAfterWebImport(
        result([
          {
            id: 'proj-1',
            name: 'Alpha',
            issue_count: 2,
            reused: false,
          },
        ])
      )
    ).toEqual({ kind: 'project', projectId: 'proj-1' });
  });

  it('opens overview when several projects were imported', () => {
    expect(
      destinationAfterWebImport(
        result([
          {
            id: 'proj-1',
            name: 'Alpha',
            issue_count: 1,
            reused: false,
          },
          {
            id: 'proj-2',
            name: 'Beta',
            issue_count: 0,
            reused: true,
          },
        ])
      )
    ).toEqual({ kind: 'overview' });
  });

  it('opens overview when the export created no projects', () => {
    expect(destinationAfterWebImport(result([]))).toEqual({
      kind: 'overview',
    });
  });
});
