import { useSyncExternalStore } from 'react';

/**
 * Archived workspaces WS is expensive (full list + summaries/diff-stats polling).
 * Keep it off until the user opens the archive UI or navigates to an archived workspace.
 */
let archivedStreamEnabled = false;
const listeners = new Set<() => void>();

function emit() {
  for (const listener of listeners) {
    listener();
  }
}

export function enableArchivedWorkspaceStream(): void {
  if (archivedStreamEnabled) return;
  archivedStreamEnabled = true;
  emit();
}

export function isArchivedWorkspaceStreamEnabled(): boolean {
  return archivedStreamEnabled;
}

export function useArchivedWorkspaceStreamEnabled(): boolean {
  return useSyncExternalStore(
    (onStoreChange) => {
      listeners.add(onStoreChange);
      return () => {
        listeners.delete(onStoreChange);
      };
    },
    () => archivedStreamEnabled,
    () => archivedStreamEnabled
  );
}
