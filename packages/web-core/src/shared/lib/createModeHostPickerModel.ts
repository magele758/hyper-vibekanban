export const LOCAL_CREATE_HOST_VALUE = 'local';

export interface CreateModeHostChoice {
  id: string;
  name: string;
  status: string;
}

/** The data service is not an executor, so it never offers "This machine". */
export function showsThisMachineOption(runtime: 'local' | 'remote'): boolean {
  return runtime === 'local';
}

export function resolveCreateModeHostValue(input: {
  runtime: 'local' | 'remote';
  executionHostId: string | null;
  routeHostId: string | null;
  hosts: readonly CreateModeHostChoice[];
}): string {
  if (input.runtime === 'local') {
    return input.executionHostId ?? LOCAL_CREATE_HOST_VALUE;
  }

  const candidate = input.executionHostId || input.routeHostId || '';
  if (!candidate) {
    return '';
  }

  const known = input.hosts.some((host) => host.id === candidate);
  return known ? candidate : '';
}

/**
 * Map a picker value to a host id.
 * - string: run on that host
 * - null: this machine (desktop only)
 * - undefined: ignore the change
 */
export function createHostIdFromPickerValue(
  runtime: 'local' | 'remote',
  value: string
): string | null | undefined {
  if (runtime === 'remote') {
    if (!value || value === LOCAL_CREATE_HOST_VALUE) {
      return undefined;
    }
    return value;
  }

  return value === LOCAL_CREATE_HOST_VALUE ? null : value;
}

/** Remote create must name an online paired host. Desktop may use this machine. */
export function isCreateHostRunnable(input: {
  runtime: 'local' | 'remote';
  hostId: string | null;
  hosts: readonly CreateModeHostChoice[];
}): boolean {
  if (input.runtime === 'local' && !input.hostId) {
    return true;
  }

  if (!input.hostId) {
    return false;
  }

  return input.hosts.some(
    (host) => host.id === input.hostId && host.status === 'online'
  );
}
