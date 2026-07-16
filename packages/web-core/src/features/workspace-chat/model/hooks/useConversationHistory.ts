import {
  ExecutionProcess,
  ExecutionProcessStatus,
  PatchType,
} from 'shared/types';
import { useExecutionProcessesContext } from '@/shared/hooks/useExecutionProcessesContext';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { streamJsonPatchEntries } from '@/shared/lib/streamJsonPatchEntries';
import type {
  AddEntryType,
  ConversationTimelineSource,
  ExecutionProcessStateStore,
  UseConversationHistoryParams,
} from '@/shared/hooks/useConversationHistory/types';
import {
  INITIAL_HISTORIC_CODING_AGENT_PROCESSES,
  LOAD_MORE_CODING_AGENT_PROCESSES,
} from '@/shared/hooks/useConversationHistory/constants';

// Result type for the new UI's conversation history hook
export interface UseConversationHistoryResult {
  /** Whether the conversation only has a single coding agent turn (no follow-ups) */
  isFirstTurn: boolean;
  /** Whether an older-history batch is currently loading (scroll-triggered) */
  isLoadingHistory: boolean;
  /** Whether older historic processes exist that have not been loaded yet */
  hasMoreHistory: boolean;
  /** Load the next batch of older history (no-op if already loading / none left) */
  loadMoreHistory: () => Promise<void>;
}

function isCodingAgentProcess(executionProcess: ExecutionProcess): boolean {
  const typ = executionProcess.executor_action.typ.type;
  return (
    typ === 'CodingAgentInitialRequest' ||
    typ === 'CodingAgentFollowUpRequest' ||
    typ === 'ReviewRequest'
  );
}

export const useConversationHistory = ({
  onTimelineUpdated,
  scopeKey,
}: UseConversationHistoryParams): UseConversationHistoryResult => {
  const {
    executionProcessesVisible: executionProcessesRaw,
    isLoading,
    isConnected,
  } = useExecutionProcessesContext();
  const executionProcesses = useRef<ExecutionProcess[]>(executionProcessesRaw);
  const displayedExecutionProcesses = useRef<ExecutionProcessStateStore>({});
  const loadedInitialEntries = useRef(false);
  const emittedEmptyInitialRef = useRef(false);
  const streamingProcessIdsRef = useRef<Set<string>>(new Set());
  const loadingMoreRef = useRef(false);
  const onTimelineUpdatedRef = useRef<
    UseConversationHistoryParams['onTimelineUpdated'] | null
  >(null);
  const previousStatusMapRef = useRef<Map<string, ExecutionProcessStatus>>(
    new Map()
  );
  const [isLoadingHistoryState, setIsLoadingHistory] = useState(false);
  const [hasMoreHistory, setHasMoreHistory] = useState(false);

  // Derive whether this is the first turn (no follow-up processes exist)
  const isFirstTurn = useMemo(() => {
    const codingAgentProcessCount = executionProcessesRaw.filter(
      (ep) =>
        ep.executor_action.typ.type === 'CodingAgentInitialRequest' ||
        ep.executor_action.typ.type === 'CodingAgentFollowUpRequest'
    ).length;
    return codingAgentProcessCount <= 1;
  }, [executionProcessesRaw]);

  const mergeIntoDisplayed = (
    mutator: (state: ExecutionProcessStateStore) => void
  ) => {
    const state = displayedExecutionProcesses.current;
    mutator(state);
  };

  // The hook owns transport, loading, and reconciliation.
  // It emits a source model that later derivation layers can transform further.

  const buildTimelineSource = useCallback(
    (
      executionProcessState: ExecutionProcessStateStore
    ): ConversationTimelineSource => ({
      executionProcessState,
      liveExecutionProcesses: executionProcesses.current,
    }),
    []
  );

  useEffect(() => {
    onTimelineUpdatedRef.current = onTimelineUpdated;
  }, [onTimelineUpdated]);

  // Keep executionProcesses up to date
  useEffect(() => {
    executionProcesses.current = executionProcessesRaw.filter(
      (ep) =>
        ep.run_reason === 'setupscript' ||
        ep.run_reason === 'cleanupscript' ||
        ep.run_reason === 'archivescript' ||
        ep.run_reason === 'codingagent'
    );
  }, [executionProcessesRaw]);

  const refreshHasMoreHistory = useCallback(() => {
    const displayed = displayedExecutionProcesses.current;
    const more = (executionProcesses.current ?? []).some(
      (p) => p.status !== ExecutionProcessStatus.running && !displayed[p.id]
    );
    setHasMoreHistory(more);
  }, []);

  const loadEntriesForHistoricExecutionProcess = (
    executionProcess: ExecutionProcess
  ) => {
    let url = '';
    if (executionProcess.executor_action.typ.type === 'ScriptRequest') {
      url = `/api/execution-processes/${executionProcess.id}/raw-logs/ws`;
    } else {
      url = `/api/execution-processes/${executionProcess.id}/normalized-logs/ws`;
    }

    return new Promise<PatchType[]>((resolve) => {
      const controller = streamJsonPatchEntries<PatchType>(url, {
        onFinished: (allEntries) => {
          controller.close();
          resolve(allEntries);
        },
        onError: (err) => {
          console.warn(
            `Error loading entries for historic execution process ${executionProcess.id}`,
            err
          );
          controller.close();
          resolve([]);
        },
      });
    });
  };

  const patchWithKey = (
    patch: PatchType,
    executionProcessId: string,
    index: number
  ) => {
    return {
      ...patch,
      patchKey: `${executionProcessId}:${index}`,
      executionProcessId,
    };
  };

  const getActiveAgentProcesses = (): ExecutionProcess[] => {
    return (
      executionProcesses?.current.filter(
        (p) =>
          p.status === ExecutionProcessStatus.running &&
          p.run_reason !== 'devserver'
      ) ?? []
    );
  };

  const emitEntries = useCallback(
    (
      executionProcessState: ExecutionProcessStateStore,
      addEntryType: AddEntryType,
      loading: boolean
    ) => {
      const timelineSource = buildTimelineSource(executionProcessState);
      let modifiedAddEntryType = addEntryType;

      const latestEntry = Object.values(executionProcessState)
        .sort(
          (a, b) =>
            new Date(
              a.executionProcess.created_at as unknown as string
            ).getTime() -
            new Date(
              b.executionProcess.created_at as unknown as string
            ).getTime()
        )
        .flatMap((processState) => processState.entries)
        .at(-1);

      if (
        latestEntry?.type === 'NORMALIZED_ENTRY' &&
        latestEntry.content.entry_type.type === 'tool_use' &&
        latestEntry.content.entry_type.tool_name === 'ExitPlanMode'
      ) {
        modifiedAddEntryType = 'plan';
      }

      onTimelineUpdatedRef.current?.(
        timelineSource,
        modifiedAddEntryType,
        loading
      );
    },
    [buildTimelineSource]
  );

  // This emits its own events as they are streamed
  const loadRunningAndEmit = useCallback(
    (executionProcess: ExecutionProcess): Promise<void> => {
      return new Promise((resolve, reject) => {
        let url = '';
        if (executionProcess.executor_action.typ.type === 'ScriptRequest') {
          url = `/api/execution-processes/${executionProcess.id}/raw-logs/ws`;
        } else {
          url = `/api/execution-processes/${executionProcess.id}/normalized-logs/ws`;
        }
        const controller = streamJsonPatchEntries<PatchType>(url, {
          onEntries(entries) {
            const patchesWithKey = entries.map((entry, index) =>
              patchWithKey(entry, executionProcess.id, index)
            );
            mergeIntoDisplayed((state) => {
              state[executionProcess.id] = {
                executionProcess,
                entries: patchesWithKey,
              };
            });
            emitEntries(displayedExecutionProcesses.current, 'running', false);
          },
          onFinished: () => {
            emitEntries(displayedExecutionProcesses.current, 'running', false);
            controller.close();
            resolve();
          },
          onError: () => {
            controller.close();
            reject();
          },
        });
      });
    },
    [emitEntries]
  );

  // Sometimes it can take a few seconds for the stream to start, wrap the loadRunningAndEmit method
  const loadRunningAndEmitWithBackoff = useCallback(
    async (executionProcess: ExecutionProcess) => {
      for (let i = 0; i < 20; i++) {
        try {
          await loadRunningAndEmit(executionProcess);
          break;
        } catch (_) {
          await new Promise((resolve) => setTimeout(resolve, 500));
        }
      }
    },
    [loadRunningAndEmit]
  );

  /**
   * Load historic processes newest-first until `maxCodingAgentProcesses` turn
   * processes are included, plus the setup script immediately older than the
   * last included turn (so the turn's setup placeholder is not missing).
   */
  const loadHistoricTurnProcesses = useCallback(
    async (
      maxCodingAgentProcesses: number,
      options?: { skipAlreadyDisplayed?: boolean }
    ): Promise<ExecutionProcessStateStore> => {
      const localDisplayedExecutionProcesses: ExecutionProcessStateStore = {};
      const skipAlreadyDisplayed = options?.skipAlreadyDisplayed ?? false;

      if (!executionProcesses?.current) return localDisplayedExecutionProcesses;

      let codingAgentCount = 0;
      let reachedTurnLimit = false;

      for (const executionProcess of [
        ...executionProcesses.current,
      ].reverse()) {
        if (executionProcess.status === ExecutionProcessStatus.running) {
          continue;
        }

        if (
          skipAlreadyDisplayed &&
          displayedExecutionProcesses.current[executionProcess.id]
        ) {
          continue;
        }

        if (reachedTurnLimit) {
          // After the turn budget, only pull the immediately older setup script(s).
          if (executionProcess.run_reason !== 'setupscript') {
            break;
          }
        }

        const entries =
          await loadEntriesForHistoricExecutionProcess(executionProcess);
        const entriesWithKey = entries.map((e, idx) =>
          patchWithKey(e, executionProcess.id, idx)
        );

        localDisplayedExecutionProcesses[executionProcess.id] = {
          executionProcess,
          entries: entriesWithKey,
        };

        if (isCodingAgentProcess(executionProcess)) {
          codingAgentCount += 1;
          if (codingAgentCount >= maxCodingAgentProcesses) {
            reachedTurnLimit = true;
          }
        }
      }

      return localDisplayedExecutionProcesses;
    },
    [executionProcesses]
  );

  const ensureProcessVisible = useCallback((p: ExecutionProcess) => {
    mergeIntoDisplayed((state) => {
      if (!state[p.id]) {
        state[p.id] = {
          executionProcess: {
            id: p.id,
            created_at: p.created_at,
            updated_at: p.updated_at,
            executor_action: p.executor_action,
          },
          entries: [],
        };
      }
    });
  }, []);

  const idListKey = useMemo(
    () => executionProcessesRaw?.map((p) => p.id).join(','),
    [executionProcessesRaw]
  );

  const idStatusKey = useMemo(
    () => executionProcessesRaw?.map((p) => `${p.id}:${p.status}`).join(','),
    [executionProcessesRaw]
  );

  const loadMoreHistory = useCallback(async () => {
    if (loadingMoreRef.current || !loadedInitialEntries.current) return;

    const hasUnloaded = (executionProcesses.current ?? []).some(
      (p) =>
        p.status !== ExecutionProcessStatus.running &&
        !displayedExecutionProcesses.current[p.id]
    );
    if (!hasUnloaded) {
      setHasMoreHistory(false);
      return;
    }

    loadingMoreRef.current = true;
    setIsLoadingHistory(true);
    try {
      const batch = await loadHistoricTurnProcesses(
        LOAD_MORE_CODING_AGENT_PROCESSES,
        { skipAlreadyDisplayed: true }
      );
      const loadedIds = Object.keys(batch);
      if (loadedIds.length === 0) {
        setHasMoreHistory(false);
        return;
      }

      mergeIntoDisplayed((state) => {
        Object.assign(state, batch);
      });
      emitEntries(displayedExecutionProcesses.current, 'historic', false);
      refreshHasMoreHistory();
    } finally {
      loadingMoreRef.current = false;
      setIsLoadingHistory(false);
    }
  }, [emitEntries, loadHistoricTurnProcesses, refreshHasMoreHistory]);

  // Clean up entries for processes that have been removed (e.g., after reset)
  useEffect(() => {
    if (isLoading || !isConnected) return;
    const visibleProcessIds = new Set(executionProcessesRaw.map((p) => p.id));
    const displayedIds = Object.keys(displayedExecutionProcesses.current);
    let changed = false;

    for (const id of displayedIds) {
      if (!visibleProcessIds.has(id)) {
        delete displayedExecutionProcesses.current[id];
        changed = true;
      }
    }

    if (changed) {
      emitEntries(displayedExecutionProcesses.current, 'historic', false);
      refreshHasMoreHistory();
    }
  }, [
    idListKey,
    executionProcessesRaw,
    emitEntries,
    isLoading,
    isConnected,
    refreshHasMoreHistory,
  ]);

  useEffect(() => {
    displayedExecutionProcesses.current = {};
    loadedInitialEntries.current = false;
    emittedEmptyInitialRef.current = false;
    streamingProcessIdsRef.current.clear();
    previousStatusMapRef.current.clear();
    loadingMoreRef.current = false;
    setHasMoreHistory(false);
    setIsLoadingHistory(false);
    emitEntries(displayedExecutionProcesses.current, 'initial', true);
  }, [scopeKey, emitEntries]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      if (loadedInitialEntries.current) return;

      if (isLoading) return;

      if (executionProcesses.current.length === 0) {
        if (emittedEmptyInitialRef.current) return;
        emittedEmptyInitialRef.current = true;
        emitEntries(displayedExecutionProcesses.current, 'initial', false);
        setHasMoreHistory(false);
        return;
      }

      emittedEmptyInitialRef.current = false;

      const allInitialEntries = await loadHistoricTurnProcesses(
        INITIAL_HISTORIC_CODING_AGENT_PROCESSES
      );
      if (cancelled) return;
      loadedInitialEntries.current = true;
      mergeIntoDisplayed((state) => {
        Object.assign(state, allInitialEntries);
      });
      emitEntries(displayedExecutionProcesses.current, 'initial', false);
      refreshHasMoreHistory();
    })();
    return () => {
      cancelled = true;
    };
  }, [
    scopeKey,
    idListKey,
    isLoading,
    loadHistoricTurnProcesses,
    emitEntries,
    refreshHasMoreHistory,
  ]); // include idListKey so new processes trigger reload

  useEffect(() => {
    const activeProcesses = getActiveAgentProcesses();
    if (activeProcesses.length === 0) return;

    for (const activeProcess of activeProcesses) {
      if (!displayedExecutionProcesses.current[activeProcess.id]) {
        const runningOrInitial =
          Object.keys(displayedExecutionProcesses.current).length > 1
            ? 'running'
            : 'initial';
        ensureProcessVisible(activeProcess);
        emitEntries(
          displayedExecutionProcesses.current,
          runningOrInitial,
          false
        );
      }

      if (
        activeProcess.status === ExecutionProcessStatus.running &&
        !streamingProcessIdsRef.current.has(activeProcess.id)
      ) {
        streamingProcessIdsRef.current.add(activeProcess.id);
        loadRunningAndEmitWithBackoff(activeProcess).finally(() => {
          streamingProcessIdsRef.current.delete(activeProcess.id);
        });
      }
    }
  }, [
    scopeKey,
    idStatusKey,
    emitEntries,
    ensureProcessVisible,
    loadRunningAndEmitWithBackoff,
  ]);

  useEffect(() => {
    if (!executionProcessesRaw) return;

    const processesToReload: ExecutionProcess[] = [];

    for (const process of executionProcessesRaw) {
      const previousStatus = previousStatusMapRef.current.get(process.id);
      const currentStatus = process.status;

      if (
        previousStatus === ExecutionProcessStatus.running &&
        currentStatus !== ExecutionProcessStatus.running &&
        displayedExecutionProcesses.current[process.id]
      ) {
        processesToReload.push(process);
      }

      previousStatusMapRef.current.set(process.id, currentStatus);
    }

    if (processesToReload.length === 0) return;

    (async () => {
      let anyUpdated = false;

      for (const process of processesToReload) {
        const entries = await loadEntriesForHistoricExecutionProcess(process);
        if (entries.length === 0) continue;

        const entriesWithKey = entries.map((e, idx) =>
          patchWithKey(e, process.id, idx)
        );

        mergeIntoDisplayed((state) => {
          state[process.id] = {
            executionProcess: process,
            entries: entriesWithKey,
          };
        });
        anyUpdated = true;
      }

      if (anyUpdated) {
        emitEntries(displayedExecutionProcesses.current, 'running', false);
      }
      refreshHasMoreHistory();
    })();
  }, [idStatusKey, executionProcessesRaw, emitEntries, refreshHasMoreHistory]);

  // If an execution process is removed, remove it from the state
  useEffect(() => {
    if (!executionProcessesRaw) return;

    const removedProcessIds = Object.keys(
      displayedExecutionProcesses.current
    ).filter((id) => !executionProcessesRaw.some((p) => p.id === id));

    if (removedProcessIds.length > 0) {
      mergeIntoDisplayed((state) => {
        removedProcessIds.forEach((id) => {
          delete state[id];
        });
      });
      refreshHasMoreHistory();
    }
  }, [scopeKey, idListKey, executionProcessesRaw, refreshHasMoreHistory]);

  // Keep hasMore in sync when the process list changes after initial load
  useEffect(() => {
    if (!loadedInitialEntries.current) return;
    refreshHasMoreHistory();
  }, [idListKey, refreshHasMoreHistory]);

  return {
    isFirstTurn,
    isLoadingHistory: isLoadingHistoryState,
    hasMoreHistory,
    loadMoreHistory,
  };
};
