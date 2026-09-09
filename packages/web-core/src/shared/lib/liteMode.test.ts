import { describe, expect, it } from 'vitest';
import {
  groupWorkspaceIdsByRepo,
  isLiteAllowedPath,
  liteCanonicalProjectPath,
  workspaceMatchesRepoFilter,
} from './liteMode';

const NO_REPO = '__no_repo__';

describe('isLiteAllowedPath', () => {
  it('allows workspace and overview routes', () => {
    expect(isLiteAllowedPath('/')).toBe(true);
    expect(isLiteAllowedPath('/workspaces')).toBe(true);
    expect(isLiteAllowedPath('/workspaces/create')).toBe(true);
    expect(isLiteAllowedPath('/workspaces/abc')).toBe(true);
    expect(isLiteAllowedPath('/overview')).toBe(true);
    expect(isLiteAllowedPath('/imported')).toBe(true);
    expect(isLiteAllowedPath('/settings')).toBe(true);
    expect(isLiteAllowedPath('/projects')).toBe(true);
    expect(isLiteAllowedPath('/projects/p1')).toBe(true);
    expect(isLiteAllowedPath('/projects/p1/workspaces/create/d1')).toBe(true);
  });

  it('blocks remaining cloud routes', () => {
    expect(isLiteAllowedPath('/agents')).toBe(false);
    expect(isLiteAllowedPath('/workforce')).toBe(false);
    expect(isLiteAllowedPath('/notifications')).toBe(false);
    expect(isLiteAllowedPath('/export')).toBe(false);
    expect(isLiteAllowedPath('/hosts/h1/workspaces')).toBe(false);
    expect(isLiteAllowedPath('/onboarding')).toBe(false);
  });
});

describe('liteCanonicalProjectPath', () => {
  it('keeps the project root and collapses cloud sub-routes', () => {
    expect(liteCanonicalProjectPath('/projects/p1')).toBe(null);
    expect(liteCanonicalProjectPath('/projects/p1/inbox')).toBe('/projects/p1');
    expect(
      liteCanonicalProjectPath('/projects/p1/issues/i1/workspaces/w1')
    ).toBe('/projects/p1');
  });
});

describe('groupWorkspaceIdsByRepo', () => {
  it('groups workspace ids by repo', () => {
    const grouped = groupWorkspaceIdsByRepo([
      { workspace_id: 'w1', repo_id: 'r1' },
      { workspace_id: 'w2', repo_id: 'r1' },
      { workspace_id: 'w3', repo_id: 'r2' },
    ]);

    expect(grouped.get('r1')).toEqual(['w1', 'w2']);
    expect(grouped.get('r2')).toEqual(['w3']);
  });
});

describe('workspaceMatchesRepoFilter', () => {
  const byRepo = groupWorkspaceIdsByRepo([
    { workspace_id: 'w1', repo_id: 'r1' },
    { workspace_id: 'w2', repo_id: 'r2' },
  ]);

  it('keeps all workspaces when no filter is set', () => {
    expect(workspaceMatchesRepoFilter('w1', [], byRepo, NO_REPO)).toBe(true);
    expect(workspaceMatchesRepoFilter('orphan', [], byRepo, NO_REPO)).toBe(
      true
    );
  });

  it('matches a linked workspace to its repo', () => {
    expect(workspaceMatchesRepoFilter('w1', ['r1'], byRepo, NO_REPO)).toBe(
      true
    );
    expect(workspaceMatchesRepoFilter('w1', ['r2'], byRepo, NO_REPO)).toBe(
      false
    );
  });

  it('matches unlinked workspaces only when the no-repo option is selected', () => {
    expect(
      workspaceMatchesRepoFilter('orphan', [NO_REPO], byRepo, NO_REPO)
    ).toBe(true);
    expect(workspaceMatchesRepoFilter('w1', [NO_REPO], byRepo, NO_REPO)).toBe(
      false
    );
  });
});
