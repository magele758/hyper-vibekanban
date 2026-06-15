import type { AppDestination } from '@/shared/lib/routes/appNavigation';
import { getDestinationHostId } from '@/shared/lib/routes/appNavigation';
import { isRelayLocalApiTransport } from '@/shared/lib/localApiTransport';

/**
 * On remote-web, prefer the route-scoped relay host when present and fall back
 * to the active paired host for project kanban routes.
 */
export function resolveRelayLocalStreamHostId(
  destination: AppDestination | null,
  fallbackHostId: string | null
): string | null {
  if (!isRelayLocalApiTransport()) {
    return fallbackHostId;
  }

  return getDestinationHostId(destination) ?? fallbackHostId;
}

export function isRelayLocalStreamEnabled(
  destination: AppDestination | null,
  fallbackHostId: string | null,
  enabled: boolean
): boolean {
  if (!enabled) {
    return false;
  }

  if (!isRelayLocalApiTransport()) {
    return true;
  }

  return resolveRelayLocalStreamHostId(destination, fallbackHostId) !== null;
}
