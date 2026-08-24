import { useQuery } from '@tanstack/react-query';
import { useWorkspaceContext } from '@/shared/hooks/useWorkspaceContext';
import { makeLocalApiRequest } from '@/shared/lib/localApiTransport';
import type { TrajectoryResponse, ApiResponse } from 'shared/types';
import { TrajectoryTimeline } from './TrajectoryTimeline';

export function TrajectoryContentContainer() {
  const { selectedSessionId } = useWorkspaceContext();

  const { data, isLoading, error } = useQuery({
    queryKey: ['trajectory', selectedSessionId, 'v2'],
    queryFn: async () => {
      if (!selectedSessionId) throw new Error('No session ID');
      const response = await makeLocalApiRequest(
        `/api/sessions/${selectedSessionId}/trajectory?include_entries=false`
      );
      if (!response.ok) {
        throw new Error('Failed to fetch trajectory');
      }
      const json = (await response.json()) as ApiResponse<TrajectoryResponse>;
      return json.data;
    },
    enabled: !!selectedSessionId,
  });

  if (!selectedSessionId) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-low">No session selected</div>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-low">Loading trajectory...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-red-400">
          Error: {error instanceof Error ? error.message : 'Unknown error'}
        </div>
      </div>
    );
  }

  if (!data) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-low">No trajectory data</div>
      </div>
    );
  }

  return <TrajectoryTimeline data={data} />;
}
