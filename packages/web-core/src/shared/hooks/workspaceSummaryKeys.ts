import { getHostRequestScopeQueryKey } from '@/shared/lib/hostRequestScope';

export const workspaceSummaryKeys = {
  all: ['workspace-summaries'] as const,
  byArchived: (archived: boolean, hostId: string | null = null) =>
    [
      'workspace-summaries',
      getHostRequestScopeQueryKey(hostId),
      archived ? 'archived' : 'active',
    ] as const,
};

export const workspaceDiffStatsKeys = {
  byWorkspaceIds: (
    archived: boolean,
    hostId: string | null,
    workspaceIds: readonly string[]
  ) =>
    [
      'workspace-diff-stats',
      getHostRequestScopeQueryKey(hostId),
      archived ? 'archived' : 'active',
      [...workspaceIds].sort().join(','),
    ] as const,
};
