import type { CreateRemoteSessionResponse } from 'shared/remote-types';
import type {
  FinishSpake2EnrollmentRequest,
  FinishSpake2EnrollmentResponse,
  RefreshRelaySigningSessionResponse,
  StartSpake2EnrollmentRequest,
  StartSpake2EnrollmentResponse,
} from 'shared/types';
import { getAuthRuntime } from '@/shared/lib/auth/runtime';

export interface RelaySigningSessionRefreshPayload {
  client_id: string;
  timestamp: number;
  nonce: string;
  signature_b64: string;
}

const BUILD_TIME_API_BASE = import.meta.env.VITE_VK_SHARED_API_BASE || '';
const BUILD_TIME_RELAY_API_BASE = import.meta.env.VITE_RELAY_API_BASE_URL || '';
const USE_REMOTE_API_BASE_FALLBACK = !BUILD_TIME_RELAY_API_BASE;

let _relayApiBase: string = BUILD_TIME_RELAY_API_BASE || BUILD_TIME_API_BASE;

/** Dev/self-host: baked relay URL may use LAN IP while the page is opened via Tailscale IP/DNS. */
function isSelfHostedDevHostname(hostname: string): boolean {
  if (
    hostname === 'localhost' ||
    hostname === '127.0.0.1' ||
    hostname === '[::1]'
  ) {
    return true;
  }
  if (/^100\.(6[4-9]|[7-9]\d|1[01]\d|12[0-7])\./.test(hostname)) {
    return true;
  }
  if (hostname.endsWith('.ts.net')) {
    return true;
  }
  if (/^(10\.|192\.168\.|172\.(1[6-9]|2\d|3[01])\.)/.test(hostname)) {
    return true;
  }
  return false;
}

export function setRelayApiBase(base: string | null | undefined) {
  if (base) {
    _relayApiBase = base;
  }
}

export function getRelayApiUrl(): string {
  return _relayApiBase;
}

/** Relay runs on a separate port from Remote; derive it for LAN / self-host. */
export function resolveDefaultRelayApiBase(
  remoteApiBase?: string | null
): string {
  const relayPort = import.meta.env.VITE_RELAY_PORT || '18082';

  if (typeof window !== 'undefined') {
    const { protocol, hostname } = window.location;

    // Front-door HTTPS pages (desktop localhost h2 / mobile Tailscale): the
    // relay is only reachable via its dedicated HTTPS front door, never the raw
    // http relay port. Trust the build-time relay base (set by vk-start to that
    // front door) instead of deriving a raw-port origin → avoids mixed-content.
    if (
      protocol === 'https:' &&
      BUILD_TIME_RELAY_API_BASE.startsWith('https:')
    ) {
      try {
        const baked = new URL(BUILD_TIME_RELAY_API_BASE);
        if (
          baked.hostname !== hostname &&
          (hostname.endsWith('.ts.net') || isSelfHostedDevHostname(hostname))
        ) {
          if (hostname.endsWith('.ts.net')) {
            const relayDoorPort =
              import.meta.env.VITE_TAILSCALE_RELAY_HTTPS_PORT || '18443';
            return `${protocol}//${hostname}:${relayDoorPort}`;
          }
          return `${protocol}//${hostname}:${relayPort}`;
        }
      } catch {
        // fall through to baked base
      }
      return BUILD_TIME_RELAY_API_BASE;
    }

    const liveRelayOrigin = `${protocol}//${hostname}:${relayPort}`;

    if (BUILD_TIME_RELAY_API_BASE) {
      try {
        const baked = new URL(BUILD_TIME_RELAY_API_BASE);
        if (
          baked.hostname === hostname ||
          baked.hostname === 'localhost' ||
          isSelfHostedDevHostname(baked.hostname)
        ) {
          return liveRelayOrigin;
        }
        return BUILD_TIME_RELAY_API_BASE;
      } catch {
        return liveRelayOrigin;
      }
    }

    return liveRelayOrigin;
  }

  if (BUILD_TIME_RELAY_API_BASE) {
    return BUILD_TIME_RELAY_API_BASE;
  }

  if (remoteApiBase) {
    try {
      const url = new URL(remoteApiBase);
      url.port = String(relayPort);
      return url.origin;
    } catch {
      // fall through
    }
  }

  return BUILD_TIME_API_BASE;
}

export function syncRelayApiBaseWithRemote(base: string | null | undefined) {
  if (USE_REMOTE_API_BASE_FALLBACK) {
    setRelayApiBase(resolveDefaultRelayApiBase(base));
  }
}

export async function createRemoteSession(
  hostId: string
): Promise<CreateRemoteSessionResponse> {
  const response = await makeAuthenticatedRequest(
    getRelayApiUrl(),
    `/v1/relay/create/${hostId}`,
    { method: 'POST' }
  );
  if (!response.ok) {
    throw await parseErrorResponse(
      response,
      'Failed to create relay session auth code'
    );
  }

  return (await response.json()) as CreateRemoteSessionResponse;
}

export function buildRemoteSessionBaseUrl(
  hostId: string,
  browserSessionId: string
): string {
  const relayBase = getRelayApiUrl().replace(/\/+$/, '');
  return `${relayBase}/v1/relay/h/${hostId}/s/${browserSessionId}`;
}

export async function startRelaySpake2Enrollment(
  hostId: string,
  sessionId: string,
  payload: StartSpake2EnrollmentRequest
): Promise<StartSpake2EnrollmentResponse> {
  const response = await makeAuthenticatedRelaySessionRequest(
    hostId,
    sessionId,
    '/api/relay-auth/server/spake2/start',
    { method: 'POST', body: JSON.stringify(payload) }
  );

  return parseLocalApiResponse(response, 'Failed to start pairing.');
}

export async function finishRelaySpake2Enrollment(
  hostId: string,
  sessionId: string,
  payload: FinishSpake2EnrollmentRequest
): Promise<FinishSpake2EnrollmentResponse> {
  const response = await makeAuthenticatedRelaySessionRequest(
    hostId,
    sessionId,
    '/api/relay-auth/server/spake2/finish',
    { method: 'POST', body: JSON.stringify(payload) }
  );

  return parseLocalApiResponse(response, 'Failed to finish pairing.');
}

export async function refreshRelaySigningSession(
  hostId: string,
  sessionId: string,
  payload: RelaySigningSessionRefreshPayload
): Promise<RefreshRelaySigningSessionResponse> {
  const response = await makeAuthenticatedRelaySessionRequest(
    hostId,
    sessionId,
    '/api/relay-auth/server/signing-session/refresh',
    { method: 'POST', body: JSON.stringify(payload) }
  );

  return parseLocalApiResponse(
    response,
    'Failed to refresh relay signing session.'
  );
}

async function makeAuthenticatedRelaySessionRequest(
  hostId: string,
  sessionId: string,
  path: string,
  options: RequestInit = {}
): Promise<Response> {
  const baseUrl = buildRemoteSessionBaseUrl(hostId, sessionId);
  return makeAuthenticatedRequest(baseUrl, path, options);
}

async function makeAuthenticatedRequest(
  baseUrl: string,
  path: string,
  options: RequestInit = {},
  retryOn401 = true
): Promise<Response> {
  const authRuntime = getAuthRuntime();
  const token = await authRuntime.getToken();
  if (!token) {
    throw new Error('Not authenticated');
  }

  const headers = new Headers(options.headers ?? {});
  if (!headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json');
  }
  headers.set('Authorization', `Bearer ${token}`);
  headers.set('X-Client-Version', __APP_VERSION__);
  headers.set('X-Client-Type', 'frontend');

  const response = await fetch(`${baseUrl}${path}`, {
    ...options,
    headers,
    credentials: 'include',
  });

  if (response.status === 401 && retryOn401) {
    const newToken = await authRuntime.triggerRefresh();
    if (newToken) {
      headers.set('Authorization', `Bearer ${newToken}`);
      return fetch(`${baseUrl}${path}`, {
        ...options,
        headers,
        credentials: 'include',
      });
    }

    throw new Error('Session expired. Please log in again.');
  }

  return response;
}

async function parseErrorResponse(
  response: Response,
  fallbackMessage: string
): Promise<Error> {
  try {
    const body = await response.json();
    const message = body.error || body.message || fallbackMessage;
    return new Error(`${message} (${response.status} ${response.statusText})`);
  } catch {
    return new Error(
      `${fallbackMessage} (${response.status} ${response.statusText})`
    );
  }
}

interface LocalApiSuccess<T> {
  success: true;
  data: T;
}

interface LocalApiFailure {
  success: false;
  message?: string;
}

type LocalApiEnvelope<T> = LocalApiSuccess<T> | LocalApiFailure;

async function parseLocalApiResponse<T>(
  response: Response,
  fallbackMessage: string
): Promise<T> {
  if (!response.ok) {
    throw new Error(await extractErrorMessage(response, fallbackMessage));
  }

  const body = (await response.json()) as LocalApiEnvelope<T>;
  if (!body.success) {
    throw new Error(body.message || fallbackMessage);
  }

  return body.data;
}

async function extractErrorMessage(
  response: Response,
  fallbackMessage: string
): Promise<string> {
  const rawBody = await response.text().catch(() => '');

  if (rawBody.includes('No active relay')) {
    return (
      '本机 Relay 未连接。请在本机运行 vk-start（或 pnpm run dev），' +
      '用 13001 登录 Remote 并保持运行后，再在 Remote/手机端配对或打开 workspace。'
    );
  }

  if (rawBody) {
    try {
      const body = JSON.parse(rawBody) as {
        message?: string;
        error?: string;
      };
      if (typeof body.message === 'string') {
        return body.message;
      }
      if (typeof body.error === 'string') {
        return body.error;
      }
    } catch {
      if (rawBody.length < 200) {
        return rawBody;
      }
    }
  }

  return `${fallbackMessage} (${response.status})`;
}
