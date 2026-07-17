import { useCallback, useEffect, useRef, useSyncExternalStore } from 'react';
import { produce } from 'immer';
import type { Operation } from 'rfc6902';
import { applyUpsertPatch } from '@/shared/lib/jsonPatch';
import { openLocalApiWebSocket } from '@/shared/lib/localApiTransport';

export type SharedJsonPatchSnapshot<T> = {
  data: T | undefined;
  isConnected: boolean;
  isInitialized: boolean;
  error: string | null;
};

type SharedEntry<T extends object> = {
  endpoint: string;
  initialData: () => T;
  refCount: number;
  ws: WebSocket | null;
  retryTimer: number | null;
  retryAttempts: number;
  finished: boolean;
  opening: boolean;
  snapshot: SharedJsonPatchSnapshot<T>;
  dataRef: T | undefined;
};

const sharedEntries = new Map<string, SharedEntry<object>>();
const listenersByEndpoint = new Map<string, Set<() => void>>();

const EMPTY_SNAPSHOT: SharedJsonPatchSnapshot<never> = {
  data: undefined,
  isConnected: false,
  isInitialized: false,
  error: null,
};

function emit(endpoint: string) {
  const listeners = listenersByEndpoint.get(endpoint);
  if (!listeners) return;
  for (const listener of listeners) {
    listener();
  }
}

function setSnapshot<T extends object>(
  entry: SharedEntry<T>,
  patch: Partial<SharedJsonPatchSnapshot<T>>
) {
  entry.snapshot = { ...entry.snapshot, ...patch };
  emit(entry.endpoint);
}

function scheduleReconnect<T extends object>(entry: SharedEntry<T>) {
  if (entry.retryTimer !== null || entry.refCount <= 0) return;
  const delay = Math.min(8000, 1000 * Math.pow(2, entry.retryAttempts));
  entry.retryTimer = window.setTimeout(() => {
    entry.retryTimer = null;
    void ensureOpen(entry);
  }, delay);
}

async function ensureOpen<T extends object>(entry: SharedEntry<T>) {
  if (entry.refCount <= 0 || entry.ws || entry.opening || entry.finished) {
    return;
  }

  entry.opening = true;
  try {
    const ws = await openLocalApiWebSocket(entry.endpoint);
    if (entry.refCount <= 0) {
      ws.close();
      return;
    }

    entry.ws = ws;
    entry.finished = false;

    ws.onopen = () => {
      entry.retryAttempts = 0;
      if (entry.retryTimer !== null) {
        window.clearTimeout(entry.retryTimer);
        entry.retryTimer = null;
      }
      setSnapshot(entry, { isConnected: true, error: null });
    };

    ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data as string) as
          | { JsonPatch: Operation[] }
          | { Ready: true }
          | { finished: boolean };

        if ('JsonPatch' in msg) {
          const patches = msg.JsonPatch;
          if (!patches.length) return;
          if (!entry.dataRef) {
            entry.dataRef = entry.initialData();
          }
          const next = produce(entry.dataRef, (draft) => {
            applyUpsertPatch(draft, patches);
          });
          entry.dataRef = next;
          setSnapshot(entry, { data: next });
        }

        if ('Ready' in msg) {
          setSnapshot(entry, { isInitialized: true, error: null });
        }

        if ('finished' in msg) {
          entry.finished = true;
          ws.close(1000, 'finished');
          entry.ws = null;
          setSnapshot(entry, { isConnected: false });
        }
      } catch {
        setSnapshot(entry, { error: 'Failed to process stream update' });
      }
    };

    ws.onerror = () => {
      /* onclose handles retry */
    };

    ws.onclose = (evt) => {
      entry.ws = null;
      setSnapshot(entry, { isConnected: false });
      if (
        entry.refCount <= 0 ||
        entry.finished ||
        (evt?.code === 1000 && evt?.wasClean)
      ) {
        return;
      }
      entry.retryAttempts += 1;
      if (!entry.dataRef && entry.retryAttempts > 6) {
        setSnapshot(entry, { error: 'Connection failed' });
      }
      scheduleReconnect(entry);
    };
  } catch {
    entry.retryAttempts += 1;
    scheduleReconnect(entry);
  } finally {
    entry.opening = false;
  }
}

function retainSharedStream<T extends object>(
  endpoint: string,
  initialData: () => T
): void {
  let entry = sharedEntries.get(endpoint) as SharedEntry<T> | undefined;
  if (!entry) {
    entry = {
      endpoint,
      initialData,
      refCount: 0,
      ws: null,
      retryTimer: null,
      retryAttempts: 0,
      finished: false,
      opening: false,
      snapshot: {
        data: undefined,
        isConnected: false,
        isInitialized: false,
        error: null,
      },
      dataRef: undefined,
    };
    sharedEntries.set(endpoint, entry as SharedEntry<object>);
  }

  entry.refCount += 1;
  if (entry.refCount === 1) {
    entry.finished = false;
    entry.dataRef = undefined;
    entry.snapshot = {
      data: undefined,
      isConnected: false,
      isInitialized: false,
      error: null,
    };
    void ensureOpen(entry);
  }
}

function releaseSharedStream(endpoint: string) {
  const entry = sharedEntries.get(endpoint);
  if (!entry) return;

  entry.refCount -= 1;
  if (entry.refCount > 0) return;

  if (entry.retryTimer !== null) {
    window.clearTimeout(entry.retryTimer);
    entry.retryTimer = null;
  }
  if (entry.ws) {
    entry.ws.onopen = null;
    entry.ws.onmessage = null;
    entry.ws.onerror = null;
    entry.ws.onclose = null;
    entry.ws.close();
    entry.ws = null;
  }
  sharedEntries.delete(endpoint);
}

/**
 * Shared JSON-patch WebSocket stream keyed by endpoint.
 * Multiple hook instances reuse one connection (approvals / same session processes).
 */
export function useSharedJsonPatchWsStream<T extends object>(
  endpoint: string | undefined,
  enabled: boolean,
  initialData: () => T
): SharedJsonPatchSnapshot<T> {
  const initialDataRef = useRef(initialData);
  initialDataRef.current = initialData;

  useEffect(() => {
    if (!enabled || !endpoint) return;
    retainSharedStream(endpoint, () => initialDataRef.current());
    return () => {
      releaseSharedStream(endpoint);
    };
  }, [enabled, endpoint]);

  const subscribe = useCallback(
    (onStoreChange: () => void) => {
      if (!endpoint) return () => {};
      let listeners = listenersByEndpoint.get(endpoint);
      if (!listeners) {
        listeners = new Set();
        listenersByEndpoint.set(endpoint, listeners);
      }
      listeners.add(onStoreChange);
      return () => {
        listeners!.delete(onStoreChange);
        if (listeners!.size === 0) {
          listenersByEndpoint.delete(endpoint);
        }
      };
    },
    [endpoint]
  );

  const getSnapshot = useCallback(() => {
    if (!enabled || !endpoint) {
      return EMPTY_SNAPSHOT as SharedJsonPatchSnapshot<T>;
    }
    const entry = sharedEntries.get(endpoint) as SharedEntry<T> | undefined;
    return entry?.snapshot ?? (EMPTY_SNAPSHOT as SharedJsonPatchSnapshot<T>);
  }, [enabled, endpoint]);

  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}
