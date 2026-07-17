import { useCallback, useMemo } from 'react';
import { useJsonPatchWsStream } from '@/shared/hooks/useJsonPatchWsStream';
import { useHostId } from '@/shared/providers/HostIdProvider';
import type { ExecutionProcess } from 'shared/types';

type ExecutionProcessState = {
  execution_processes: Record<string, ExecutionProcess>;
};

interface UseExecutionProcessesResult {
  executionProcesses: ExecutionProcess[];
  executionProcessesById: Record<string, ExecutionProcess>;
  isAttemptRunning: boolean;
  isLoading: boolean;
  isConnected: boolean;
  error: string | null;
}

/**
 * Stream execution processes for a session via WebSocket (JSON Patch) and expose as array + map.
 * Server sends initial snapshot: replace /execution_processes with an object keyed by id.
 * Live updates arrive at /execution_processes/<id> via add/replace/remove operations.
 *
 * Prefer `useExecutionProcessesContext` inside workspace layout to avoid duplicate sockets.
 */
export const useExecutionProcesses = (
  sessionId: string | undefined,
  opts?: { showSoftDeleted?: boolean }
): UseExecutionProcessesResult => {
  const hostId = useHostId();
  const showSoftDeleted = opts?.showSoftDeleted;

  const endpoint = useMemo(() => {
    if (!sessionId) return undefined;
    const apiBasePath = hostId ? `/api/host/${hostId}` : '/api';
    const params = new URLSearchParams({ session_id: sessionId });
    if (typeof showSoftDeleted === 'boolean') {
      params.set('show_soft_deleted', String(showSoftDeleted));
    }
    return `${apiBasePath}/execution-processes/stream/session/ws?${params.toString()}`;
  }, [hostId, sessionId, showSoftDeleted]);

  const initialData = useCallback(
    (): ExecutionProcessState => ({ execution_processes: {} }),
    []
  );

  const { data, isConnected, isInitialized, error } =
    useJsonPatchWsStream<ExecutionProcessState>(
      endpoint,
      !!sessionId,
      initialData
    );

  const executionProcesses = useMemo(() => {
    const streamed = Object.values(data?.execution_processes ?? {}).sort(
      (a, b) =>
        new Date(a.created_at as unknown as string).getTime() -
        new Date(b.created_at as unknown as string).getTime()
    );

    return sessionId
      ? streamed.filter(
          (executionProcess) => executionProcess.session_id === sessionId
        )
      : streamed;
  }, [data?.execution_processes, sessionId]);

  const executionProcessesById = useMemo(() => {
    return executionProcesses.reduce<Record<string, ExecutionProcess>>(
      (processesById, executionProcess) => {
        processesById[executionProcess.id] = executionProcess;
        return processesById;
      },
      {}
    );
  }, [executionProcesses]);

  const isAttemptRunning = useMemo(
    () =>
      executionProcesses.some(
        (process) =>
          (process.run_reason === 'codingagent' ||
            process.run_reason === 'setupscript' ||
            process.run_reason === 'cleanupscript' ||
            process.run_reason === 'archivescript') &&
          process.status === 'running'
      ),
    [executionProcesses]
  );

  const isLoading = !!sessionId && !isInitialized && !error;

  return {
    executionProcesses,
    executionProcessesById,
    isAttemptRunning,
    isLoading,
    isConnected,
    error,
  };
};
