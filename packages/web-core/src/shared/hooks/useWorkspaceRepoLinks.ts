import { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { workspacesApi } from '@/shared/lib/api';
import {
  groupWorkspaceIdsByRepo,
  type WorkspaceRepoLink,
} from '@/shared/lib/liteMode';

export function useWorkspaceRepoLinks(enabled = true) {
  const query = useQuery({
    queryKey: ['workspace-repo-links'],
    queryFn: () => workspacesApi.listRepoLinks(),
    enabled,
    staleTime: 15_000,
  });

  const links = useMemo<WorkspaceRepoLink[]>(
    () =>
      (query.data ?? []).map((link) => ({
        workspace_id: link.workspace_id,
        repo_id: link.repo_id,
      })),
    [query.data]
  );

  const workspaceIdsByRepo = useMemo(
    () => groupWorkspaceIdsByRepo(links),
    [links]
  );

  return {
    links,
    workspaceIdsByRepo,
    isLoading: query.isLoading,
  };
}
