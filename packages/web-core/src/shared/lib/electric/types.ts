/**
 * Error type for Electric sync operations.
 * Wraps errors from Electric's onError callback (HTTP errors, network failures, etc.)
 */
export interface SyncError {
  /** HTTP status code if available */
  status?: number;
  /** Error message */
  message: string;
}

/**
 * Configuration options for creating Electric collections.
 */
export interface CollectionConfig {
  /** Callback for sync errors */
  onError?: (error: SyncError) => void;
  /** Time to wait for Electric before falling back to REST */
  readyTimeoutMs?: number;
  /**
   * When false, fetch a snapshot and close the stream instead of holding a
   * live long-poll. Used on HTTP/1.1 to keep the ~6 connection slots free.
   * @default true
   */
  subscribe?: boolean;
}

/**
 * Result of an optimistic mutation operation.
 * Contains a promise that resolves when the backend confirms the change.
 */
export interface MutationResult {
  /** Promise that resolves when the mutation is confirmed by the backend */
  persisted: Promise<void>;
}

/**
 * Result of an insert operation, including the created row data.
 */
export interface InsertResult<TRow> {
  /** The optimistically created row with generated ID */
  data: TRow;
  /** Promise that resolves with the synced row (including server-generated fields) when confirmed by backend */
  persisted: Promise<TRow>;
}
