import { isLocalRelayHostId } from '@/shared/lib/localRelayHost';
import { getCurrentHostId } from '@/shared/providers/HostIdProvider';

export type LocalApiHostScope = 'current' | 'explicit' | 'none';

export interface LocalApiRequestOptions extends RequestInit {
  hostScope?: LocalApiHostScope;
  hostId?: string | null;
  relayHostId?: string | null;
}

export interface LocalApiWebSocketOptions {
  hostScope?: LocalApiHostScope;
  hostId?: string | null;
  relayHostId?: string | null;
}

export interface LocalApiTransport {
  request: (
    pathOrUrl: string,
    init?: LocalApiRequestOptions
  ) => Promise<Response>;
  openWebSocket: (
    pathOrUrl: string,
    options?: LocalApiWebSocketOptions
  ) => Promise<WebSocket> | WebSocket;
}

const LOCAL_ONLY_API_PREFIXES = [
  '/api/open-remote-editor/',
  '/api/relay-auth/server/',
  '/api/relay-auth/client/',
];

function isAbsoluteUrl(pathOrUrl: string): boolean {
  return /^https?:\/\//i.test(pathOrUrl) || /^wss?:\/\//i.test(pathOrUrl);
}

function toPathAndQuery(pathOrUrl: string): string {
  if (isAbsoluteUrl(pathOrUrl)) {
    const url = new URL(pathOrUrl);
    return `${url.pathname}${url.search}`;
  }
  return pathOrUrl.startsWith('/') ? pathOrUrl : `/${pathOrUrl}`;
}

function toAbsoluteWsUrl(pathOrUrl: string): string {
  if (/^wss?:\/\//i.test(pathOrUrl)) return pathOrUrl;
  if (/^https?:\/\//i.test(pathOrUrl)) return pathOrUrl.replace(/^http/i, 'ws');

  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  const path = pathOrUrl.startsWith('/') ? pathOrUrl : `/${pathOrUrl}`;
  return `${protocol}//${window.location.host}${path}`;
}

/** Collapse `/api/host/{own_id}/...` to `/api/...` for this machine. */
function stripOwnHostPrefix(pathOrUrl: string): string {
  const pathAndQuery = toPathAndQuery(pathOrUrl);
  const qIndex = pathAndQuery.indexOf('?');
  const pathname = qIndex >= 0 ? pathAndQuery.slice(0, qIndex) : pathAndQuery;
  const search = qIndex >= 0 ? pathAndQuery.slice(qIndex) : '';
  const match = pathname.match(/^\/api\/host\/([^/]+)(\/.*)?$/);
  if (!match) return pathOrUrl;
  const [, hostId, rest = ''] = match;
  if (!isLocalRelayHostId(hostId)) return pathOrUrl;
  return `/api${rest}${search}`;
}

function scopeLocalApiPath(pathOrUrl: string, hostId: string | null): string {
  const normalized = stripOwnHostPrefix(pathOrUrl);
  // This machine's own host id must stay on plain /api/* — routing through
  // /api/host/{own_id} would bounce via Relay and break local WS streams.
  if (!hostId || isLocalRelayHostId(hostId)) return normalized;
  const path = toPathAndQuery(normalized);
  // These endpoints must always hit the local backend because they rely on
  // local-only credentials/state.
  if (LOCAL_ONLY_API_PREFIXES.some((prefix) => path.startsWith(prefix))) {
    return normalized;
  }

  if (!path.startsWith('/api/') || path.startsWith('/api/host/'))
    return normalized;

  const suffix = path.slice('/api'.length);
  return `/api/host/${hostId}${suffix}`;
}

function resolveScopedPath(
  pathOrUrl: string,
  options: {
    hostScope?: LocalApiHostScope;
    hostId?: string | null;
  } = {}
): string {
  const hostScope = options.hostScope ?? 'current';

  if (hostScope === 'none') {
    return stripOwnHostPrefix(pathOrUrl);
  }

  if (hostScope === 'explicit') {
    return scopeLocalApiPath(pathOrUrl, options.hostId ?? null);
  }

  return scopeLocalApiPath(pathOrUrl, getCurrentHostId());
}

const defaultTransport: LocalApiTransport = {
  request: (pathOrUrl, init = {}) => {
    const {
      hostScope: _hostScope,
      hostId: _hostId,
      relayHostId: _relayHostId,
      ...requestInit
    } = init;
    return fetch(pathOrUrl, requestInit);
  },
  openWebSocket: (pathOrUrl, _options = {}) =>
    new WebSocket(toAbsoluteWsUrl(pathOrUrl)),
};

let transport: LocalApiTransport = defaultTransport;

export function isRelayLocalApiTransport(): boolean {
  return transport !== defaultTransport;
}

export function setLocalApiTransport(nextTransport: LocalApiTransport | null) {
  transport = nextTransport ?? defaultTransport;
}

export async function makeLocalApiRequest(
  pathOrUrl: string,
  init: LocalApiRequestOptions = {}
): Promise<Response> {
  return transport.request(resolveScopedPath(pathOrUrl, init), init);
}

export async function openLocalApiWebSocket(
  pathOrUrl: string,
  options: LocalApiWebSocketOptions = {}
): Promise<WebSocket> {
  return transport.openWebSocket(
    resolveScopedPath(pathOrUrl, options),
    options
  );
}
