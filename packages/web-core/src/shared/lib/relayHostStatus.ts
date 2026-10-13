export type RelayListStatus = 'online' | 'offline' | 'unpaired';

export function relayListStatus(
  remoteStatus: string,
  paired: boolean
): RelayListStatus {
  if (!paired) {
    return 'unpaired';
  }
  return remoteStatus === 'online' ? 'online' : 'offline';
}

export function hostIsLinked(
  hostId: string,
  indexedHostIds: ReadonlySet<string>,
  serverHostIds: ReadonlySet<string>
): boolean {
  return indexedHostIds.has(hostId) || serverHostIds.has(hostId);
}
