import { useQuery } from '@tanstack/react-query';
import { useWorkspaceContext } from '@/shared/hooks/useWorkspaceContext';
import { makeLocalApiRequest } from '@/shared/lib/localApiTransport';
import type {
  TrajectoryResponse,
  TrajectorySegment,
  ExecutionProcessStatus,
  ApiResponse,
} from 'shared/types';

const STATUS_BADGE_COLORS: Record<ExecutionProcessStatus, string> = {
  completed: 'bg-green-500/20 text-green-400',
  failed: 'bg-red-500/20 text-red-400',
  killed: 'bg-orange-500/20 text-orange-400',
  running: 'bg-blue-500/20 text-blue-400',
};

export function TrajectoryContentContainer() {
  const { selectedSessionId } = useWorkspaceContext();

  const { data, isLoading, error } = useQuery({
    queryKey: ['trajectory', selectedSessionId],
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

  return (
    <div className="flex h-full flex-col gap-4 overflow-auto p-4">
      {/* Session Header */}
      <div className="flex flex-col gap-2 rounded-lg bg-surface-1 p-4">
        <div className="text-sm font-medium text-high">
          {data.session_name || 'Unnamed Session'}
        </div>
        {data.executor && (
          <div className="text-xs text-low">Executor: {data.executor}</div>
        )}
      </div>

      {/* Completeness Overview */}
      <div className="flex flex-col gap-2 rounded-lg bg-surface-1 p-4">
        <div className="text-xs font-medium text-medium">Completeness</div>
        <div className="grid grid-cols-2 gap-2 text-xs">
          <div>
            <span className="text-low">Total Processes:</span>{' '}
            <span className="text-high">
              {data.completeness.total_processes}
            </span>
          </div>
          <div>
            <span className="text-low">With Logs:</span>{' '}
            <span className="text-high">{data.completeness.with_logs}</span>
          </div>
          <div>
            <span className="text-low">Dropped:</span>{' '}
            <span className="text-high">{data.completeness.dropped}</span>
          </div>
          <div>
            <span className="text-low">Missing Logs:</span>{' '}
            <span className="text-high">
              {data.completeness.missing_logs.length}
            </span>
          </div>
        </div>
      </div>

      {/* Totals */}
      {data.totals && (
        <div className="flex flex-col gap-2 rounded-lg bg-surface-1 p-4">
          <div className="text-xs font-medium text-medium">Totals</div>
          <div className="flex flex-col gap-2">
            {Object.entries(data.totals.entries_by_type).length > 0 && (
              <div>
                <div className="mb-1 text-xs text-low">Entries by Type:</div>
                <div className="flex flex-wrap gap-2">
                  {Object.entries(data.totals.entries_by_type).map(
                    ([type, count]) => (
                      <div
                        key={type}
                        className="rounded bg-surface-2 px-2 py-1 text-xs"
                      >
                        <span className="text-low">{type}:</span>{' '}
                        <span className="text-high">{count}</span>
                      </div>
                    )
                  )}
                </div>
              </div>
            )}
            {Object.entries(data.totals.tool_calls_by_status).length > 0 && (
              <div>
                <div className="mb-1 text-xs text-low">
                  Tool Calls by Status:
                </div>
                <div className="flex flex-wrap gap-2">
                  {Object.entries(data.totals.tool_calls_by_status).map(
                    ([status, count]) => (
                      <div
                        key={status}
                        className="rounded bg-surface-2 px-2 py-1 text-xs"
                      >
                        <span className="text-low">{status}:</span>{' '}
                        <span className="text-high">{count}</span>
                      </div>
                    )
                  )}
                </div>
              </div>
            )}
            {data.totals.last_token_usage && (
              <div>
                <div className="mb-1 text-xs text-low">Last Token Usage:</div>
                <div className="text-xs">
                  <span className="text-low">Total:</span>{' '}
                  <span className="text-high">
                    {data.totals.last_token_usage.total_tokens.toLocaleString()}
                  </span>
                  {' / '}
                  <span className="text-low">Context:</span>{' '}
                  <span className="text-high">
                    {data.totals.last_token_usage.model_context_window.toLocaleString()}
                  </span>
                </div>
              </div>
            )}
          </div>
        </div>
      )}

      {/* Segments Timeline */}
      <div className="flex flex-col gap-2">
        <div className="text-xs font-medium text-medium">
          Execution Timeline ({data.segments.length} segments)
        </div>
        <div className="flex flex-col gap-2">
          {data.segments.map((segment: TrajectorySegment) => (
            <div
              key={segment.execution_process_id}
              className="flex flex-col gap-2 rounded-lg bg-surface-1 p-3"
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <span
                    className={`rounded px-2 py-0.5 text-xs ${
                      STATUS_BADGE_COLORS[segment.status]
                    }`}
                  >
                    {segment.status}
                  </span>
                  <span className="text-xs text-medium">
                    {segment.run_reason}
                  </span>
                  {segment.dropped && (
                    <span className="rounded bg-gray-500/20 px-2 py-0.5 text-xs text-gray-400">
                      dropped
                    </span>
                  )}
                </div>
                {segment.exit_code !== null &&
                  segment.exit_code !== undefined && (
                    <span className="text-xs text-low">
                      exit: {String(segment.exit_code)}
                    </span>
                  )}
              </div>

              <div className="text-xs text-low">
                Started: {new Date(segment.started_at).toLocaleString()}
                {segment.completed_at && (
                  <>
                    {' '}
                    • Completed:{' '}
                    {new Date(segment.completed_at).toLocaleString()}
                  </>
                )}
              </div>

              <div className="flex items-center gap-4 text-xs text-low">
                <span>Entries: {segment.entry_count}</span>
                <span>Has Logs: {segment.has_logs ? 'Yes' : 'No'}</span>
              </div>

              {segment.final_message && (
                <div className="mt-2 rounded bg-surface-2 p-2 text-xs text-medium">
                  {segment.final_message}
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
