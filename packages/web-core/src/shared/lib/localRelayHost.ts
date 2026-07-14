import { listRelayHosts } from '@/shared/lib/remoteApi';

let localRelayHostId: string | null = null;
let resolvePromise: Promise<string | null> | null = null;

/** Host id for this machine's own relay registration, if known. */
export function getLocalRelayHostId(): string | null {
  return localRelayHostId;
}

export function setLocalRelayHostId(hostId: string | null): void {
  localRelayHostId = hostId;
}

/**
 * Resolve and cache this machine's relay host id from Remote `/v1/hosts`.
 * Used so local-web can skip `/api/host/{own_id}` (which would bounce via Relay).
 */
export async function refreshLocalRelayHostId(
  machineId: string | null | undefined
): Promise<string | null> {
  if (!machineId) {
    localRelayHostId = null;
    return null;
  }

  if (!resolvePromise) {
    resolvePromise = (async () => {
      try {
        const hosts = await listRelayHosts();
        const match = hosts.find((host) => host.machine_id === machineId);
        localRelayHostId = match?.id ?? null;
        return localRelayHostId;
      } catch {
        localRelayHostId = null;
        return null;
      } finally {
        resolvePromise = null;
      }
    })();
  }

  return resolvePromise;
}

export function isLocalRelayHostId(hostId: string | null | undefined): boolean {
  return Boolean(hostId && localRelayHostId && hostId === localRelayHostId);
}
