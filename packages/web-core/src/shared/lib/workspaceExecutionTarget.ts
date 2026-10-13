import type { AppRuntime } from '@/shared/hooks/useAppRuntime';
import type { LocalApiRequestOptions } from '@/shared/lib/localApiTransport';

export interface WorkspaceExecutionTarget {
  runtime: AppRuntime;
  hostId: string | null;
}

/**
 * Where a workspace create request must go.
 * Remote pages relay `/api/workspaces` to the chosen host. They never leave
 * the path on the data service origin.
 */
export function workspaceExecutionRequestOptions(
  target: WorkspaceExecutionTarget
): Pick<LocalApiRequestOptions, 'hostScope' | 'hostId' | 'relayHostId'> {
  if (target.runtime === 'remote') {
    if (!target.hostId) {
      throw new Error(
        'Select an online host. The data service does not run workspaces.'
      );
    }

    return {
      hostScope: 'none',
      relayHostId: target.hostId,
    };
  }

  if (target.hostId) {
    return {
      hostScope: 'explicit',
      hostId: target.hostId,
    };
  }

  return {
    hostScope: 'none',
  };
}
