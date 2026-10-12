export interface IssueWorkspaceCreateHost {
  id: string;
  status: string;
}

export interface IssueWorkspaceCreateHostInput {
  runtime: 'local' | 'remote';
  routeHostId?: string | null;
  executionHostId?: string | null;
  pageHostId?: string | null;
  hosts: readonly IssueWorkspaceCreateHost[];
}

function isOnline(
  hostId: string | null | undefined,
  hosts: readonly IssueWorkspaceCreateHost[]
): boolean {
  if (!hostId) {
    return false;
  }

  // Status not loaded yet: keep an id the user or the current page already has.
  if (hosts.length === 0) {
    return true;
  }

  return hosts.some((host) => host.id === hostId && host.status === 'online');
}

/**
 * Host that should run a workspace created from an existing issue.
 * Returns null for this machine, or when no online host can be chosen
 * without picking an arbitrary (often offline) machine.
 */
export function resolveIssueWorkspaceCreateHostId(
  input: IssueWorkspaceCreateHostInput
): string | null {
  const hosts = input.hosts;
  const explicit = input.routeHostId || input.executionHostId || null;

  if (isOnline(explicit, hosts)) {
    return explicit;
  }

  // Selected host is known and offline. On desktop, stay on this machine
  // instead of hosts[0] — that machine disables create when it is offline
  // or has no checkout of this repo.
  if (explicit && hosts.length > 0 && input.runtime === 'local') {
    return null;
  }

  if (
    input.pageHostId &&
    input.pageHostId !== explicit &&
    isOnline(input.pageHostId, hosts)
  ) {
    return input.pageHostId;
  }

  if (input.runtime === 'remote' && hosts.length > 0) {
    const online = hosts.filter((host) => host.status === 'online');
    if (online.length === 1) {
      return online[0].id;
    }
  }

  return null;
}

/**
 * hostId argument for workspace-create navigation.
 * - string: create on that host
 * - null: do not inherit the page host
 * - undefined: keep the shell's current host
 */
export function issueCreateNavigationHostId(
  input: IssueWorkspaceCreateHostInput & { targetHostId: string | null }
): string | null | undefined {
  if (input.targetHostId) {
    return input.targetHostId;
  }

  if (input.runtime === 'local') {
    return null;
  }

  if (input.hosts.length === 0) {
    return undefined;
  }

  const pageOnline = input.hosts.some(
    (host) => host.id === input.pageHostId && host.status === 'online'
  );
  if (pageOnline) {
    return undefined;
  }

  return null;
}
