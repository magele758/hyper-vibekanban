import { useCallback, useMemo } from 'react';
import { useQuery, keepPreviousData } from '@tanstack/react-query';
import { useJsonPatchWsStream } from '@/shared/hooks/useJsonPatchWsStream';
import {
  workspaceSummaryKeys,
  workspaceDiffStatsKeys,
} from '@/shared/hooks/workspaceSummaryKeys';
import { makeLocalApiRequest } from '@/shared/lib/localApiTransport';
import { isLocalRelayHostId } from '@/shared/lib/localRelayHost';
import { useCurrentAppDestination } from '@/shared/hooks/useCurrentAppDestination';
import { useHostId } from '@/shared/providers/HostIdProvider';
import {
  isRelayLocalStreamEnabled,
  resolveRelayLocalStreamHostId,
} from '@/shared/lib/relayLocalStreams';
import type {
  WorkspaceWithStatus,
  WorkspaceSummary,
  WorkspaceSummaryResponse,
  WorkspaceDiffStatsResponse,
  ApiResponse,
} from 'shared/types';

// UI-specific workspace type for sidebar display
export interface SidebarWorkspace {
  id: string;
  name: string;
  branch: string;
  createdAt: string;
  updatedAt: string;
  description: string;
  filesChanged?: number;
  linesAdded?: number;
  linesRemoved?: number;
  isRunning?: boolean;
  isPinned?: boolean;
  isArchived?: boolean;
  hasPendingApproval?: boolean;
  hasRunningDevServer?: boolean;
  hasUnseenActivity?: boolean;
  latestProcessCompletedAt?: string;
  latestProcessStatus?: 'running' | 'completed' | 'failed' | 'killed';
  prStatus?: 'open' | 'merged' | 'closed' | 'unknown';
  prNumber?: number;
  prUrl?: string;
}

// Keep the old export name for backwards compatibility
export type Workspace = SidebarWorkspace;

export interface UseWorkspacesResult {
  workspaces: SidebarWorkspace[];
  archivedWorkspaces: SidebarWorkspace[];
  isLoading: boolean;
  isConnected: boolean;
  error: string | null;
}

// State shape from the WebSocket stream
type WorkspacesState = {
  workspaces: Record<string, WorkspaceWithStatus>;
};

// Transform WorkspaceWithStatus to SidebarWorkspace, optionally merging summary data
function toSidebarWorkspace(
  ws: WorkspaceWithStatus,
  summary?: WorkspaceSummary,
  diffStats?: {
    files_changed: number;
    lines_added: number;
    lines_removed: number;
  }
): SidebarWorkspace {
  return {
    id: ws.id,
    name: ws.name ?? ws.branch, // Use name if available, fallback to branch
    branch: ws.branch,
    createdAt: ws.created_at,
    updatedAt: ws.updated_at,
    description: '',
    filesChanged:
      diffStats?.files_changed ?? summary?.files_changed ?? undefined,
    linesAdded: diffStats?.lines_added ?? summary?.lines_added ?? undefined,
    linesRemoved:
      diffStats?.lines_removed ?? summary?.lines_removed ?? undefined,
    // Real data from stream
    isRunning: ws.is_running,
    isPinned: ws.pinned,
    isArchived: ws.archived,
    // Additional data from summary
    hasPendingApproval: summary?.has_pending_approval,
    hasRunningDevServer: summary?.has_running_dev_server,
    hasUnseenActivity: summary?.has_unseen_turns,
    latestProcessCompletedAt: summary?.latest_process_completed_at ?? undefined,
    latestProcessStatus: summary?.latest_process_status ?? undefined,
    prStatus: summary?.pr_status ?? undefined,
    prNumber:
      summary?.pr_number != null ? Number(summary.pr_number) : undefined,
    prUrl: summary?.pr_url ?? undefined,
  };
}

export const workspaceKeys = {
  all: ['workspaces'] as const,
};

// workspaceSummaryKeys is imported from @/shared/hooks/workspaceSummaryKeys

// Fetch workspace summaries from the API by archived status
async function fetchWorkspaceSummariesByArchived(
  archived: boolean,
  hostId: string | null
): Promise<Map<string, WorkspaceSummary>> {
  try {
    const basePath =
      hostId && !isLocalRelayHostId(hostId) ? `/api/host/${hostId}` : '/api';
    const response = await makeLocalApiRequest(
      `${basePath}/workspaces/summaries`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ archived }),
      }
    );

    if (!response.ok) {
      console.warn('Failed to fetch workspace summaries:', response.status);
      return new Map();
    }

    const data: ApiResponse<WorkspaceSummaryResponse> = await response.json();
    if (!data.success || !data.data?.summaries) {
      return new Map();
    }

    const map = new Map<string, WorkspaceSummary>();
    for (const summary of data.data.summaries) {
      map.set(summary.workspace_id, summary);
    }
    return map;
  } catch (err) {
    console.warn('Error fetching workspace summaries:', err);
    return new Map();
  }
}

async function fetchWorkspaceDiffStats(
  workspaceIds: string[],
  hostId: string | null
): Promise<
  Map<
    string,
    { files_changed: number; lines_added: number; lines_removed: number }
  >
> {
  if (workspaceIds.length === 0) {
    return new Map();
  }

  try {
    const basePath =
      hostId && !isLocalRelayHostId(hostId) ? `/api/host/${hostId}` : '/api';
    const response = await makeLocalApiRequest(
      `${basePath}/workspaces/diff-stats`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ workspace_ids: workspaceIds }),
      }
    );

    if (!response.ok) {
      console.warn('Failed to fetch workspace diff stats:', response.status);
      return new Map();
    }

    const data: ApiResponse<WorkspaceDiffStatsResponse> = await response.json();
    if (!data.success || !data.data?.stats) {
      return new Map();
    }

    const map = new Map<
      string,
      { files_changed: number; lines_added: number; lines_removed: number }
    >();
    for (const entry of data.data.stats) {
      map.set(entry.workspace_id, {
        files_changed: entry.files_changed,
        lines_added: entry.lines_added,
        lines_removed: entry.lines_removed,
      });
    }
    return map;
  } catch (err) {
    console.warn('Error fetching workspace diff stats:', err);
    return new Map();
  }
}

export function useWorkspaces(): UseWorkspacesResult {
  const destination = useCurrentAppDestination();
  const fallbackHostId = useHostId();
  const streamHostId = resolveRelayLocalStreamHostId(
    destination,
    fallbackHostId
  );
  const workspaceStreamsEnabled = isRelayLocalStreamEnabled(
    destination,
    fallbackHostId,
    true
  );

  // Two separate WebSocket connections: one for active, one for archived
  // No limit param - we fetch all and slice on frontend so backfill works when archiving
  // Own-machine host id is treated as local (/api), not /api/host/{id}.
  const useHostPrefix =
    Boolean(streamHostId) && !isLocalRelayHostId(streamHostId);
  const apiBasePath = useHostPrefix ? `/api/host/${streamHostId}` : '/api';
  const activeEndpoint = `${apiBasePath}/workspaces/streams/ws?archived=false`;
  const archivedEndpoint = `${apiBasePath}/workspaces/streams/ws?archived=true`;

  const initialData = useCallback(
    (): WorkspacesState => ({ workspaces: {} }),
    []
  );

  const {
    data: activeData,
    isConnected: activeIsConnected,
    isInitialized: activeIsInitialized,
    error: activeError,
  } = useJsonPatchWsStream<WorkspacesState>(
    activeEndpoint,
    workspaceStreamsEnabled,
    initialData
  );

  const {
    data: archivedData,
    isConnected: archivedIsConnected,
    isInitialized: archivedIsInitialized,
    error: archivedError,
  } = useJsonPatchWsStream<WorkspacesState>(
    archivedEndpoint,
    workspaceStreamsEnabled,
    initialData
  );

  // Wait for both streams to be initialized before fetching summaries
  // Fetch summaries for active workspaces
  const { data: activeSummaries = new Map<string, WorkspaceSummary>() } =
    useQuery({
      queryKey: workspaceSummaryKeys.byArchived(false, streamHostId),
      queryFn: () => fetchWorkspaceSummariesByArchived(false, streamHostId),
      enabled: activeIsInitialized,
      staleTime: 1000,
      refetchInterval: 15000,
      refetchOnWindowFocus: false,
      refetchOnMount: 'always',
      placeholderData: keepPreviousData,
    });

  // Fetch summaries for archived workspaces
  const { data: archivedSummaries = new Map<string, WorkspaceSummary>() } =
    useQuery({
      queryKey: workspaceSummaryKeys.byArchived(true, streamHostId),
      queryFn: () => fetchWorkspaceSummariesByArchived(true, streamHostId),
      enabled: archivedIsInitialized,
      staleTime: 1000,
      refetchInterval: 15000,
      refetchOnWindowFocus: false,
      refetchOnMount: 'always',
      placeholderData: keepPreviousData,
    });

  const activeWorkspaceIds = useMemo(
    () => Object.keys(activeData?.workspaces ?? {}).sort(),
    [activeData?.workspaces]
  );
  const archivedWorkspaceIds = useMemo(
    () => Object.keys(archivedData?.workspaces ?? {}).sort(),
    [archivedData?.workspaces]
  );

  const { data: activeDiffStats = new Map() } = useQuery({
    queryKey: workspaceDiffStatsKeys.byWorkspaceIds(
      false,
      streamHostId,
      activeWorkspaceIds
    ),
    queryFn: () => fetchWorkspaceDiffStats(activeWorkspaceIds, streamHostId),
    enabled: activeIsInitialized && activeWorkspaceIds.length > 0,
    staleTime: 30_000,
    refetchInterval: 60_000,
    refetchOnWindowFocus: false,
    placeholderData: keepPreviousData,
  });

  const { data: archivedDiffStats = new Map() } = useQuery({
    queryKey: workspaceDiffStatsKeys.byWorkspaceIds(
      true,
      streamHostId,
      archivedWorkspaceIds
    ),
    queryFn: () => fetchWorkspaceDiffStats(archivedWorkspaceIds, streamHostId),
    enabled: archivedIsInitialized && archivedWorkspaceIds.length > 0,
    staleTime: 30_000,
    refetchInterval: 60_000,
    refetchOnWindowFocus: false,
    placeholderData: keepPreviousData,
  });

  const workspaces = useMemo(() => {
    if (!activeData?.workspaces) return [];
    return Object.values(activeData.workspaces)
      .sort((a, b) => {
        // First sort by pinned (pinned first)
        if (a.pinned !== b.pinned) {
          return a.pinned ? -1 : 1;
        }
        // Then by created_at (newest first)
        return (
          new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
        );
      })
      .map((ws) =>
        toSidebarWorkspace(
          ws,
          activeSummaries.get(ws.id),
          activeDiffStats.get(ws.id)
        )
      );
  }, [activeData, activeSummaries, activeDiffStats]);

  const archivedWorkspaces = useMemo(() => {
    if (!archivedData?.workspaces) return [];
    return Object.values(archivedData.workspaces)
      .sort((a, b) => {
        // First sort by pinned (pinned first)
        if (a.pinned !== b.pinned) {
          return a.pinned ? -1 : 1;
        }
        // Then by created_at (newest first)
        return (
          new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
        );
      })
      .map((ws) =>
        toSidebarWorkspace(
          ws,
          archivedSummaries.get(ws.id),
          archivedDiffStats.get(ws.id)
        )
      );
  }, [archivedData, archivedSummaries, archivedDiffStats]);

  // isLoading is true when we haven't received initial data from either stream
  const isLoading =
    workspaceStreamsEnabled && (!activeIsInitialized || !archivedIsInitialized);

  // Combined connection status
  const isConnected = activeIsConnected && archivedIsConnected;

  // Combined error (show first error if any)
  const error = activeError || archivedError;

  return {
    workspaces,
    archivedWorkspaces,
    isLoading,
    isConnected,
    error,
  };
}
