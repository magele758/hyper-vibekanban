import { useCallback } from 'react';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { useAppRuntime } from '@/shared/hooks/useAppRuntime';
import { useCurrentKanbanRouteState } from '@/shared/hooks/useCurrentKanbanRouteState';
import { useProjectContext } from '@/shared/hooks/useProjectContext';
import { setExecutionHostId } from '@/shared/lib/executionHostContext';
import { setRelayHostFallback } from '@/shared/lib/relayHostFallback';
import { persistWorkspaceCreateDraft } from '@/shared/lib/workspaceCreateState';
import { getCurrentHostId } from '@/shared/providers/HostIdProvider';
import type { CreateModeInitialState } from '@/shared/types/createMode';

export function useProjectWorkspaceCreateDraft() {
  const { projectId } = useProjectContext();
  const appNavigation = useAppNavigation();
  const routeState = useCurrentKanbanRouteState();
  const runtime = useAppRuntime();

  const openWorkspaceCreateFromState = useCallback(
    async (
      initialState: CreateModeInitialState,
      options?: { issueId?: string | null; hostId?: string | null }
    ): Promise<string | null> => {
      if (!projectId) return null;

      const hasHostOverride = options != null && 'hostId' in options;
      const hostId = hasHostOverride ? (options.hostId ?? null) : undefined;

      // Point scratch/API at the target host before the draft is saved.
      // A null override means this machine, so an offline selection is not
      // inherited by the create page (that disables the submit button).
      if (typeof hostId === 'string') {
        if (runtime === 'local') {
          setExecutionHostId(hostId);
        }
        if (!getCurrentHostId()) {
          setRelayHostFallback(hostId);
        }
      } else if (hostId === null && runtime === 'local') {
        setExecutionHostId(null);
        if (!getCurrentHostId()) {
          setRelayHostFallback(null);
        }
      }

      const draftId = await persistWorkspaceCreateDraft(
        initialState,
        crypto.randomUUID(),
        runtime
      );
      if (!draftId) {
        return null;
      }

      const issueId =
        options?.issueId ??
        initialState.linkedIssue?.issueId ??
        routeState.issueId ??
        null;
      if (issueId) {
        appNavigation.goToProjectIssueWorkspaceCreate(
          projectId,
          issueId,
          draftId,
          undefined,
          hostId
        );
      } else {
        appNavigation.goToProjectWorkspaceCreate(
          projectId,
          draftId,
          undefined,
          hostId
        );
      }

      return draftId;
    },
    [projectId, appNavigation, routeState.issueId, runtime]
  );

  return {
    openWorkspaceCreateFromState,
  };
}
