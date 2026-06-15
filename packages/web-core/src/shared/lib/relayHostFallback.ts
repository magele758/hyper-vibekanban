let relayHostFallback: string | null = null;

/** Remote-web sets the active paired relay host for routes without /hosts/{id}. */
export function setRelayHostFallback(hostId: string | null): void {
  relayHostFallback = hostId;
}

export function getRelayHostFallback(): string | null {
  return relayHostFallback;
}
