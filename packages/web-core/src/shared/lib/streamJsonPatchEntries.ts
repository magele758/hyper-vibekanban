// streamJsonPatchEntries.ts - WebSocket JSON patch streaming utility
import { produce } from 'immer';
import type { Operation } from 'rfc6902';
import { applyUpsertPatch } from '@/shared/lib/jsonPatch';
import { openLocalApiWebSocket } from '@/shared/lib/localApiTransport';

type PatchContainer<E = unknown> = { entries: E[] };

export interface StreamOptions<E = unknown> {
  initial?: PatchContainer<E>;
  /** called after each successful patch application */
  onEntries?: (entries: E[]) => void;
  onConnect?: () => void;
  onError?: (err: unknown) => void;
  /** called once when a "finished" event is received */
  onFinished?: (entries: E[]) => void;
  /**
   * When true, a socket close without a prior `finished` message is treated as
   * success and invokes `onFinished` with the current entries (historic replay).
   * When false/omitted, that close invokes `onError` instead (live streams).
   */
  finishOnClose?: boolean;
  /**
   * When true, apply JsonPatch ops immediately instead of batching on rAF.
   * Prefer for historic replay where we only care about Finished latency.
   */
  immediateFlush?: boolean;
}

interface StreamController<E = unknown> {
  /** Current entries array (immutable snapshot) */
  getEntries(): E[];
  /** Full { entries } snapshot */
  getSnapshot(): PatchContainer<E>;
  /** Best-effort connection state */
  isConnected(): boolean;
  /** Subscribe to updates; returns an unsubscribe function */
  onChange(cb: (entries: E[]) => void): () => void;
  /** Close the stream */
  close(): void;
}

/**
 * Connect to a WebSocket endpoint that emits JSON messages containing:
 *   {"JsonPatch": [{"op": "add", "path": "/entries/0", "value": {...}}, ...]}
 *   {"Finished": ""}
 *
 * Maintains an in-memory { entries: [] } snapshot and returns a controller.
 *
 * Messages are batched per animation frame and applied using immer for
 * structural sharing, avoiding a full deep clone on every message.
 */
export function streamJsonPatchEntries<E = unknown>(
  url: string,
  opts: StreamOptions<E> = {}
): StreamController<E> {
  let connected = false;
  let closed = false;
  let finished = false;
  let ws: WebSocket | null = null;
  let snapshot: PatchContainer<E> = structuredClone(
    opts.initial ?? ({ entries: [] } as PatchContainer<E>)
  );

  const subscribers = new Set<(entries: E[]) => void>();
  if (opts.onEntries) subscribers.add(opts.onEntries);

  // --- batching state ---
  let pendingOps: Operation[] = [];
  let rafId: number | null = null;

  const notify = () => {
    for (const cb of subscribers) {
      try {
        cb(snapshot.entries);
      } catch {
        /* swallow subscriber errors */
      }
    }
  };

  const flush = () => {
    rafId = null;
    if (pendingOps.length === 0) return;

    const ops = dedupeOps(pendingOps);
    pendingOps = [];

    if (opts.immediateFlush) {
      // Historic replay: mutate in place to avoid O(n²) immer array copies.
      // Must stay synchronous — async batching races with WS close/abort and
      // can drop the entire historic payload (empty load-more).
      applyUpsertPatch(snapshot, ops);
    } else {
      snapshot = produce(snapshot, (draft) => {
        applyUpsertPatch(draft, ops);
      });
    }
    notify();
  };

  const flushPendingSync = () => {
    if (rafId !== null) {
      cancelAnimationFrame(rafId);
      rafId = null;
    }
    flush();
  };

  const handleMessage = (event: MessageEvent) => {
    try {
      const msg = JSON.parse(event.data);

      // Handle JsonPatch messages — historic flushes sync; live batches on rAF
      if (msg.JsonPatch) {
        const raw = msg.JsonPatch as Operation[];
        pendingOps.push(...raw);
        if (opts.immediateFlush) {
          flushPendingSync();
        } else if (rafId === null) {
          rafId = requestAnimationFrame(flush);
        }
      }

      // Handle Finished messages — flush synchronously before closing
      if (msg.finished !== undefined) {
        finished = true;
        flushPendingSync();
        opts.onFinished?.(snapshot.entries);
        ws?.close();
      }
    } catch (err) {
      opts.onError?.(err);
    }
  };

  void (async () => {
    try {
      const opened = await openLocalApiWebSocket(url);

      if (closed) {
        opened.close();
        return;
      }

      ws = opened;
      ws.addEventListener('open', () => {
        connected = true;
        opts.onConnect?.();
      });

      ws.addEventListener('message', handleMessage);

      ws.addEventListener('error', (err) => {
        connected = false;
        if (!finished && !closed) {
          opts.onError?.(err);
        }
      });

      ws.addEventListener('close', () => {
        connected = false;
        flushPendingSync();
        // Client `close()` already settled via the caller; do not double-fire.
        if (finished || closed) return;
        if (opts.finishOnClose) {
          finished = true;
          opts.onFinished?.(snapshot.entries);
        } else {
          opts.onError?.(new Error('WebSocket closed before finished'));
        }
      });
    } catch (error) {
      if (!closed && !finished) {
        opts.onError?.(error);
      }
    }
  })();

  return {
    getEntries(): E[] {
      return snapshot.entries;
    },
    getSnapshot(): PatchContainer<E> {
      return snapshot;
    },
    isConnected(): boolean {
      return connected;
    },
    onChange(cb: (entries: E[]) => void): () => void {
      subscribers.add(cb);
      // push current state immediately
      cb(snapshot.entries);
      return () => subscribers.delete(cb);
    },
    close(): void {
      // Flush first so abort/settle cannot discard an in-flight historic batch.
      flushPendingSync();
      closed = true;
      if (rafId !== null) {
        cancelAnimationFrame(rafId);
        rafId = null;
      }
      pendingOps = [];
      ws?.close();
      subscribers.clear();
      connected = false;
    },
  };
}

/**
 * Dedupe multiple ops that touch the same path within a batch.
 * Last write for a path wins, while preserving the overall left-to-right
 * order of the *kept* final operations.
 *
 * Example:
 *   add /entries/4, replace /entries/4  -> keep only the final replace
 */
function dedupeOps(ops: Operation[]): Operation[] {
  const lastIndexByPath = new Map<string, number>();
  ops.forEach((op, i) => lastIndexByPath.set(op.path, i));

  // Keep only the last op for each path, in ascending order of their final index
  const keptIndices = [...lastIndexByPath.values()].sort((a, b) => a - b);
  return keptIndices.map((i) => ops[i]!);
}
