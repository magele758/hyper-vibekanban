import type { QueryClient } from '@tanstack/react-query';
import type { WorkspaceSummary } from 'shared/types';
import { workspaceSummaryKeys } from '@/shared/hooks/workspaceSummaryKeys';

/** Optimistically clear unseen activity after markSeen — avoids refetching summaries. */
export function markWorkspaceSeenInSummaryCache(
  queryClient: QueryClient,
  workspaceId: string,
  hostId: string | null
): void {
  for (const archived of [false, true] as const) {
    queryClient.setQueryData<Map<string, WorkspaceSummary>>(
      workspaceSummaryKeys.byArchived(archived, hostId),
      (prev) => {
        if (!prev?.has(workspaceId)) {
          return prev;
        }
        const summary = prev.get(workspaceId);
        if (!summary?.has_unseen_turns) {
          return prev;
        }
        const next = new Map(prev);
        next.set(workspaceId, { ...summary, has_unseen_turns: false });
        return next;
      }
    );
  }
}
