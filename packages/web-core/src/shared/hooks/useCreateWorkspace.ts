import { useMutation, useQueryClient } from '@tanstack/react-query';
import { workspacesApi } from '@/shared/lib/api';
import type { WorkspaceExecutionTarget } from '@/shared/lib/workspaceExecutionTarget';
import type { CreateAndStartWorkspaceRequest } from 'shared/types';
import { workspaceSummaryKeys } from '@/shared/hooks/workspaceSummaryKeys';
import { localProjectKeys } from '@/shared/hooks/useLocalProjects';

interface CreateWorkspaceParams {
  data: CreateAndStartWorkspaceRequest;
  linkToIssue?: {
    remoteProjectId: string;
    issueId: string;
  };
  execution?: WorkspaceExecutionTarget;
}

export function useCreateWorkspace() {
  const queryClient = useQueryClient();

  const createWorkspace = useMutation({
    mutationFn: async ({
      data,
      linkToIssue,
      execution,
    }: CreateWorkspaceParams) => {
      const { workspace } = await workspacesApi.createAndStart(data, execution);

      if (linkToIssue && workspace && !data.linked_issue) {
        await workspacesApi.linkToIssue(
          workspace.id,
          linkToIssue.remoteProjectId,
          linkToIssue.issueId,
          execution
        );
      }

      return { workspace };
    },
    onSuccess: (_result, variables) => {
      // Invalidate workspace summaries so they refresh with the new workspace included
      queryClient.invalidateQueries({ queryKey: workspaceSummaryKeys.all });
      // Ensure create-mode defaults refetch the latest session/model selection.
      queryClient.invalidateQueries({ queryKey: ['workspaceCreateDefaults'] });
      queryClient.invalidateQueries({ queryKey: localProjectKeys.all });
      if (variables.data.project_id) {
        queryClient.invalidateQueries({
          queryKey: localProjectKeys.detail(variables.data.project_id),
        });
      }
    },
    onError: (err) => {
      console.error('Failed to create workspace:', err);
    },
  });

  return { createWorkspace };
}
