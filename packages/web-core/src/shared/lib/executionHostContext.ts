const STORAGE_KEY = 'vk.executionHostId';

type Listener = () => void;

let executionHostId: string | null = readStoredExecutionHostId();
const listeners = new Set<Listener>();

function readStoredExecutionHostId(): string | null {
  if (typeof window === 'undefined') {
    return null;
  }

  try {
    const value = window.localStorage.getItem(STORAGE_KEY);
    return value && value.length > 0 ? value : null;
  } catch {
    return null;
  }
}

function persistExecutionHostId(hostId: string | null): void {
  if (typeof window === 'undefined') {
    return;
  }

  try {
    if (hostId) {
      window.localStorage.setItem(STORAGE_KEY, hostId);
    } else {
      window.localStorage.removeItem(STORAGE_KEY);
    }
  } catch {
    // Ignore quota / private-mode failures; in-memory value still applies.
  }
}

function emit(): void {
  for (const listener of listeners) {
    listener();
  }
}

/** Selected remote host for creating/running workspaces from Desktop. null = this machine. */
export function getExecutionHostId(): string | null {
  return executionHostId;
}

export function setExecutionHostId(hostId: string | null): void {
  if (executionHostId === hostId) {
    return;
  }

  executionHostId = hostId;
  persistExecutionHostId(hostId);
  emit();
}

export function subscribeExecutionHostId(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
