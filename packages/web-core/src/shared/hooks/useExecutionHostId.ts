import { useCallback, useSyncExternalStore } from 'react';
import {
  getExecutionHostId,
  setExecutionHostId,
  subscribeExecutionHostId,
} from '@/shared/lib/executionHostContext';

export function useExecutionHostId(): {
  executionHostId: string | null;
  setExecutionHostId: (hostId: string | null) => void;
} {
  const executionHostId = useSyncExternalStore(
    subscribeExecutionHostId,
    getExecutionHostId,
    () => null
  );

  const updateExecutionHostId = useCallback((hostId: string | null) => {
    setExecutionHostId(hostId);
  }, []);

  return {
    executionHostId,
    setExecutionHostId: updateExecutionHostId,
  };
}
