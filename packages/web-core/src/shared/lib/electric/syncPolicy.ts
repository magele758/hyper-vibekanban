import { getRemoteApiUrl } from '@/shared/lib/remoteApi';

/**
 * HTTP/1.1 allows ~6 connections per origin. Each Electric live shape holds one
 * until the long-poll returns, so extra lives stall the board snapshot.
 * HTTPS (Caddy / Tailscale front door) is HTTP/2 and can multiplex.
 */
export const CONSTRAINED_SHAPE_READY_TIMEOUT_MS = 20_000;
export const CONSTRAINED_COLLECTION_GC_TIME_MS = 15_000;

export type ElectricShapeLane = 'live' | 'snapshot';

export function isPlainHttpUrl(url: string): boolean {
  try {
    return new URL(url).protocol === 'http:';
  } catch {
    return url.startsWith('http:') && !url.startsWith('https:');
  }
}

export function isConstrainedElectricSyncFrom(
  pageProtocol: string,
  apiBase: string
): boolean {
  if (pageProtocol === 'https:') {
    return false;
  }
  if (!apiBase) {
    return pageProtocol === 'http:';
  }
  return isPlainHttpUrl(apiBase);
}

export function isConstrainedElectricSync(): boolean {
  if (typeof window === 'undefined') {
    return false;
  }
  return isConstrainedElectricSyncFrom(
    window.location.protocol,
    getRemoteApiUrl()
  );
}

export function resolveElectricShapeSyncOptions(args: {
  constrained: boolean;
  lane: ElectricShapeLane;
  unconstrainedReadyTimeoutMs?: number;
}): { subscribe: boolean; readyTimeoutMs?: number } {
  if (!args.constrained) {
    return {
      subscribe: true,
      readyTimeoutMs: args.unconstrainedReadyTimeoutMs,
    };
  }

  return {
    subscribe: args.lane === 'live',
    readyTimeoutMs: CONSTRAINED_SHAPE_READY_TIMEOUT_MS,
  };
}

export function getElectricShapeSyncOptions(
  lane: ElectricShapeLane,
  unconstrainedReadyTimeoutMs?: number
): { subscribe: boolean; readyTimeoutMs?: number } {
  return resolveElectricShapeSyncOptions({
    constrained: isConstrainedElectricSync(),
    lane,
    unconstrainedReadyTimeoutMs,
  });
}
