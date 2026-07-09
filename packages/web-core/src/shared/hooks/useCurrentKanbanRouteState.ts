import { useMemo } from 'react';
import { useCurrentAppDestination } from '@/shared/hooks/useCurrentAppDestination';
import { useExecutionHostId } from '@/shared/hooks/useExecutionHostId';
import {
  resolveKanbanRouteState,
  type KanbanRouteState,
} from '@/shared/lib/routes/appNavigation';
import {
  buildKanbanIssueComposerKey,
  useKanbanIssueComposer,
} from '@/shared/stores/useKanbanIssueComposerStore';

export function useCurrentKanbanRouteState(): KanbanRouteState {
  const destination = useCurrentAppDestination();
  const { executionHostId } = useExecutionHostId();
  const routeState = useMemo(
    () => resolveKanbanRouteState(destination),
    [destination]
  );
  // Kanban boards (project routes) never carry a /hosts/{id} URL segment, so on
  // Desktop fold in the selected execution host. This keeps the issue composer
  // (and any other host-scoped route state) from colliding across hosts for the
  // same project. On remote-web executionHostId is always null → unchanged.
  const effectiveHostId = routeState.hostId ?? executionHostId;
  const issueComposerKey = useMemo(() => {
    if (!routeState.projectId) {
      return null;
    }

    return buildKanbanIssueComposerKey(effectiveHostId, routeState.projectId);
  }, [effectiveHostId, routeState.projectId]);
  const issueComposer = useKanbanIssueComposer(issueComposerKey);
  const isCreateMode = issueComposer !== null;

  return useMemo(
    () => ({
      ...routeState,
      hostId: effectiveHostId,
      isCreateMode,
      isPanelOpen: routeState.isPanelOpen || isCreateMode,
    }),
    [routeState, effectiveHostId, isCreateMode]
  );
}
