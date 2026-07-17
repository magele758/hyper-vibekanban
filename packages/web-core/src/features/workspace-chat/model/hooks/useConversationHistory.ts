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
  const loadMoreAbortRef = useRef<AbortController | null>(null);
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
    executionProcess: ExecutionProcess,
    options?: {
      onProgress?: (entries: PatchType[]) => void;
      signal?: AbortSignal;
    }
  ) => {
    let url = '';
    if (executionProcess.executor_action.typ.type === 'ScriptRequest') {
      url = `/api/execution-processes/${executionProcess.id}/raw-logs/ws`;
    } else {
      url = `/api/execution-processes/${executionProcess.id}/normalized-logs/ws`;
    }

    return new Promise<PatchType[]>((resolve) => {
      if (options?.signal?.aborted) {
        resolve([]);
        return;
      }

      let settled = false;
      let latestEntries: PatchType[] = [];

      const settle = (entries: PatchType[]) => {
        if (settled) return;
        settled = true;
        options?.signal?.removeEventListener('abort', onAbort);
        controller.close();
        resolve(entries);
      };

      const onAbort = () => {
        settle(latestEntries);
      };

      const controller = streamJsonPatchEntries<PatchType>(url, {
        // Historic replay: server often closes after the last patch without a
        // separate `finished` frame. Treat that as success so Promise.all
        // cannot hang the whole batch (initial / loadMore).
        finishOnClose: true,
        // Skip rAF batching — we only emit after Finished; sync flush finishes sooner.
        immediateFlush: true,
        onEntries: (entries) => {
          latestEntries = entries;
          options?.onProgress?.(entries);
        },
        onFinished: (allEntries) => {
          settle(allEntries);
        },
        onError: (err) => {
          console.warn(
            `Error loading entries for historic execution process ${executionProcess.id}`,
            err
          );
          settle(latestEntries);
        },
      });

      options?.signal?.addEventListener('abort', onAbort, { once: true });
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

  /** Reuse prior keyed entries when immer kept the underlying patch identity. */
  const keyEntriesIncremental = (
    entries: PatchType[],
    executionProcessId: string,
    previous: ReturnType<typeof patchWithKey>[] | undefined
  ) => {
    const next = new Array(entries.length);
    for (let index = 0; index < entries.length; index++) {
      const entry = entries[index]!;
      const existing = previous?.[index];
      if (
        existing &&
        existing.type === entry.type &&
        // Structural sharing from immer keeps prior entry objects stable.
        (existing as { content?: unknown }).content ===
          (entry as { content?: unknown }).content
      ) {
        next[index] = existing;
      } else {
        next[index] = patchWithKey(entry, executionProcessId, index);
      }
    }
    return next as ReturnType<typeof patchWithKey>[];
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

      // Only inspect the chronologically latest process's last entry —
      // avoid sort+flatMap of the full history on every streaming frame.
      let latestProcessTime = -Infinity;
      let latestEntry:
        | (typeof executionProcessState)[string]['entries'][number]
        | undefined;
      for (const processState of Object.values(executionProcessState)) {
        const time = Date.parse(
          String(processState.executionProcess.created_at)
        );
        if (!Number.isFinite(time) || time < latestProcessTime) continue;
        latestProcessTime = time;
        latestEntry = processState.entries.at(-1);
      }

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
            const previous =
              displayedExecutionProcesses.current[executionProcess.id]?.entries;
            const patchesWithKey = keyEntriesIncremental(
              entries,
              executionProcess.id,
              previous
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
   * Select historic processes newest-first until `maxCodingAgentProcesses`
   * coding-agent turns are included. When `includeSidecarScripts` is true,
   * also include cleanup newer than the turn and setup immediately older.
   */
  const selectHistoricTurnProcesses = useCallback(
    (
      maxCodingAgentProcesses: number,
      options?: {
        skipAlreadyDisplayed?: boolean;
        includeSidecarScripts?: boolean;
      }
    ): ExecutionProcess[] => {
      const skipAlreadyDisplayed = options?.skipAlreadyDisplayed ?? false;
      const includeSidecarScripts = options?.includeSidecarScripts ?? true;
      const toLoad: ExecutionProcess[] = [];
      let codingAgentCount = 0;
      let reachedTurnLimit = false;

      if (!executionProcesses?.current) return toLoad;

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
          if (!includeSidecarScripts) break;
          // After the turn budget, only pull the immediately older setup script(s).
          if (executionProcess.run_reason !== 'setupscript') {
            break;
          }
        }

        if (!includeSidecarScripts && !isCodingAgentProcess(executionProcess)) {
          // Agent-first path: skip cleanup/setup until deferred phase.
          continue;
        }

        toLoad.push(executionProcess);

        if (isCodingAgentProcess(executionProcess)) {
          codingAgentCount += 1;
          if (codingAgentCount >= maxCodingAgentProcesses) {
            reachedTurnLimit = true;
            if (!includeSidecarScripts) break;
          }
        }
      }

      return toLoad;
    },
    []
  );

  /**
   * Fetch log WS for a selected process list in parallel.
   * When `onLatestAgentProgress` is set, the newest coding-agent streams
   * throttled progress callbacks before Finished.
   */
  const fetchHistoricProcesses = useCallback(
    async (
      toLoad: ExecutionProcess[],
      options?: {
        signal?: AbortSignal;
        onLatestAgentProgress?: (
          executionProcess: ExecutionProcess,
          entries: PatchType[]
        ) => void;
      }
    ): Promise<ExecutionProcessStateStore> => {
      const localDisplayedExecutionProcesses: ExecutionProcessStateStore = {};
      if (toLoad.length === 0 || options?.signal?.aborted) {
        return localDisplayedExecutionProcesses;
      }

      const progressiveProcessId = options?.onLatestAgentProgress
        ? toLoad.find(isCodingAgentProcess)?.id
        : undefined;

      const loaded = await Promise.all(
        toLoad.map(async (executionProcess) => {
          const isProgressive = executionProcess.id === progressiveProcessId;
          const entries = await loadEntriesForHistoricExecutionProcess(
            executionProcess,
            {
              signal: options?.signal,
              onProgress: isProgressive
                ? (progressEntries) => {
                    options?.onLatestAgentProgress?.(
                      executionProcess,
                      progressEntries
                    );
                  }
                : undefined,
            }
          );
          return { executionProcess, entries };
        })
      );

      if (options?.signal?.aborted) return localDisplayedExecutionProcesses;

      for (const { executionProcess, entries } of loaded) {
        localDisplayedExecutionProcesses[executionProcess.id] = {
          executionProcess,
          entries: keyEntriesIncremental(
            entries,
            executionProcess.id,
            undefined
          ),
        };
      }

      return localDisplayedExecutionProcesses;
    },
    []
  );

  const loadHistoricTurnProcesses = useCallback(
    async (
      maxCodingAgentProcesses: number,
      options?: {
        skipAlreadyDisplayed?: boolean;
        includeSidecarScripts?: boolean;
        signal?: AbortSignal;
        onLatestAgentProgress?: (
          executionProcess: ExecutionProcess,
          entries: PatchType[]
        ) => void;
      }
    ): Promise<ExecutionProcessStateStore> => {
      const toLoad = selectHistoricTurnProcesses(maxCodingAgentProcesses, {
        skipAlreadyDisplayed: options?.skipAlreadyDisplayed,
        includeSidecarScripts: options?.includeSidecarScripts,
      });
      return fetchHistoricProcesses(toLoad, {
        signal: options?.signal,
        onLatestAgentProgress: options?.onLatestAgentProgress,
      });
    },
    [fetchHistoricProcesses, selectHistoricTurnProcesses]
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
    const abort = new AbortController();
    loadMoreAbortRef.current = abort;
    try {
      const batch = await loadHistoricTurnProcesses(
        LOAD_MORE_CODING_AGENT_PROCESSES,
        {
          skipAlreadyDisplayed: true,
          includeSidecarScripts: true,
          signal: abort.signal,
        }
      );
      if (abort.signal.aborted) return;

      // Ignore empty fetches so a failed/aborted WS does not mark the process
      // displayed and permanently hide "load earlier".
      const nonEmptyBatch: ExecutionProcessStateStore = {};
      for (const [id, state] of Object.entries(batch)) {
        if (state.entries.length > 0) {
          nonEmptyBatch[id] = state;
        }
      }

      const loadedIds = Object.keys(nonEmptyBatch);
      if (loadedIds.length === 0) {
        // Keep hasMore true if unloaded processes remain — user can retry.
        refreshHasMoreHistory();
        return;
      }

      mergeIntoDisplayed((state) => {
        Object.assign(state, nonEmptyBatch);
      });
      emitEntries(displayedExecutionProcesses.current, 'historic', false);
      refreshHasMoreHistory();
    } finally {
      if (loadMoreAbortRef.current === abort) {
        loadMoreAbortRef.current = null;
      }
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
    loadMoreAbortRef.current?.abort();
    loadMoreAbortRef.current = null;
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
    const abort = new AbortController();

    (async () => {
      if (loadedInitialEntries.current) return;

      // Wait for processes WS Ready (full snapshot). No extra debounce — Ready
      // already means the initial list is complete.
      if (isLoading) return;

      if (executionProcesses.current.length === 0) {
        if (emittedEmptyInitialRef.current) return;
        emittedEmptyInitialRef.current = true;
        emitEntries(displayedExecutionProcesses.current, 'initial', false);
        setHasMoreHistory(false);
        return;
      }

      emittedEmptyInitialRef.current = false;

      // Newest turn in one parallel batch (agent + adjacent setup/cleanup).
      // Single emit avoids a second prepend/layout shift after first paint.
      const initialEntries = await loadHistoricTurnProcesses(
        INITIAL_HISTORIC_CODING_AGENT_PROCESSES,
        {
          signal: abort.signal,
          includeSidecarScripts: true,
        }
      );
      if (abort.signal.aborted) return;
      loadedInitialEntries.current = true;
      mergeIntoDisplayed((state) => {
        Object.assign(state, initialEntries);
      });
      emitEntries(displayedExecutionProcesses.current, 'initial', false);
      refreshHasMoreHistory();
    })();
    return () => {
      abort.abort();
    };
  }, [
    scopeKey,
    idListKey,
    isLoading,
    loadHistoricTurnProcesses,
    emitEntries,
    refreshHasMoreHistory,
  ]);

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
    let metadataOnlyUpdate = false;

    for (const process of executionProcessesRaw) {
      const previousStatus = previousStatusMapRef.current.get(process.id);
      const currentStatus = process.status;

      if (
        previousStatus === ExecutionProcessStatus.running &&
        currentStatus !== ExecutionProcessStatus.running &&
        displayedExecutionProcesses.current[process.id]
      ) {
        const existing =
          displayedExecutionProcesses.current[process.id]?.entries ?? [];
        // Live stream already delivered the full turn — only re-fetch when
        // we have no entries (stream failed / never connected).
        if (existing.length > 0) {
          mergeIntoDisplayed((state) => {
            const current = state[process.id];
            if (current) {
              state[process.id] = {
                ...current,
                executionProcess: process,
              };
            }
          });
          metadataOnlyUpdate = true;
        } else {
          processesToReload.push(process);
        }
      }

      previousStatusMapRef.current.set(process.id, currentStatus);
    }

    if (metadataOnlyUpdate && processesToReload.length === 0) {
      emitEntries(displayedExecutionProcesses.current, 'running', false);
      refreshHasMoreHistory();
      return;
    }

    if (processesToReload.length === 0) return;

    (async () => {
      const reloaded = await Promise.all(
        processesToReload.map(async (process) => {
          const entries = await loadEntriesForHistoricExecutionProcess(process);
          return { process, entries };
        })
      );

      let anyUpdated = false;
      for (const { process, entries } of reloaded) {
        if (entries.length === 0) continue;

        const previous =
          displayedExecutionProcesses.current[process.id]?.entries;
        const entriesWithKey = keyEntriesIncremental(
          entries,
          process.id,
          previous
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
