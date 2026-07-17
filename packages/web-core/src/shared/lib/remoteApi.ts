import type {
  AttachmentUrlResponse,
  AttachmentWithBlob,
  CommitAttachmentsRequest,
  CommitAttachmentsResponse,
  ConfirmUploadRequest,
  InitUploadRequest,
  InitUploadResponse,
  ListRelayHostsResponse,
  RelayHost,
  UpdateIssueRequest,
  UpdateProjectRequest,
  UpdateProjectStatusRequest,
} from 'shared/remote-types';
import { getAuthRuntime } from '@/shared/lib/auth/runtime';
import { sha256Bytes } from '@/shared/lib/relayCrypto';
import {
  isLoopbackHostname,
  isSelfHostedDevHostname,
  syncRelayApiBaseWithRemote,
} from '@/shared/lib/relayBackendApi';

const BUILD_TIME_API_BASE = import.meta.env.VITE_VK_SHARED_API_BASE || '';

// Mutable module-level variable — overridden at runtime by ConfigProvider
// when VK_SHARED_API_BASE is set (for self-hosting support)
let _remoteApiBase: string = BUILD_TIME_API_BASE;

/**
 * Prefer the page's own origin when it is served over HTTPS (Caddy h2 front
 * door — desktop localhost or mobile Tailscale) but the backend still reports a
 * plaintext http base (LAN dev). Routing REST + Electric shapes same-origin
 * lets them multiplex over a single h2 connection instead of stalling on the
 * ~6-per-origin HTTP/1.1 limit.
 *
 * On a plain-http self-hosted/dev page, rewrite the reported base's host to the
 * current page hostname (keeping its port). vk-start bakes a single host into
 * VK_BROWSER_SHARED_API_BASE (LAN IP when Tailscale was down at startup — e.g.
 * the login autostart, which waits for Docker but not Tailscale). Without this,
 * a device opening the page over Tailscale (cellular) would be told to call the
 * unreachable LAN IP for the Remote API, so the app loads but every remote call
 * fails — even though WiFi works. Following the page host instead makes the
 * Remote API reachable via whatever host opened the page (Tailscale IP/DNS or
 * LAN), mirroring resolveDefaultRelayApiBase.
 *
 * Production safety: there apiBase is https, so neither branch matches and the
 * reported base is returned unchanged (never rewritten to window.origin).
 */
export function resolveSharedRemoteApiBase(
  apiBase: string | null | undefined
): string | null {
  const buildTimeBase = BUILD_TIME_API_BASE || null;
  if (!apiBase) {
    return buildTimeBase;
  }

  if (typeof window === 'undefined') {
    return apiBase;
  }

  // Desktop vite is http://localhost:13001 while VITE_VK_SHARED_API_BASE is the
  // Caddy h2 front door (https://localhost:13443). Prefer that https build-time
  // base over VK_BROWSER_SHARED_API_BASE (plaintext Tailscale/LAN :13000), or
  // ConfigProvider would force HTTP/1.1 and stall Electric shapes.
  if (
    buildTimeBase &&
    buildTimeBase.startsWith('https:') &&
    apiBase.startsWith('http:') &&
    window.location.protocol === 'http:' &&
    (window.location.hostname === 'localhost' ||
      window.location.hostname === '127.0.0.1')
  ) {
    try {
      const buildUrl = new URL(buildTimeBase);
      if (
        buildUrl.hostname === 'localhost' ||
        buildUrl.hostname === '127.0.0.1'
      ) {
        return buildTimeBase;
      }
    } catch {
      // fall through
    }
  }

  if (window.location.protocol === 'https:' && apiBase.startsWith('http:')) {
    return window.location.origin;
  }

  if (window.location.protocol === 'http:' && apiBase.startsWith('http:')) {
    const { hostname } = window.location;
    try {
      const baked = new URL(apiBase);
      // Rewrite same-machine LAN ↔ Tailscale hosts so a phone opening the
      // page over Tailscale does not keep a stale LAN API base. Never rewrite
      // a remote host onto localhost — worker desktops use localhost:13001
      // while Remote/Relay live on another machine (e.g. Mac Tailscale IP).
      if (
        baked.hostname !== hostname &&
        isSelfHostedDevHostname(hostname) &&
        !(isLoopbackHostname(hostname) && !isLoopbackHostname(baked.hostname))
      ) {
        baked.hostname = hostname;
        return baked.origin;
      }
    } catch {
      // fall through to the reported base
    }
  }

  return apiBase;
}

/**
 * Set the remote API base URL at runtime.
 * Called by ConfigProvider when /api/info returns a shared_api_base value.
 * No-op if base is null/undefined/empty (preserves build-time fallback).
 */
export function setRemoteApiBase(base: string | null | undefined) {
  const resolved = resolveSharedRemoteApiBase(base);
  _remoteApiBase = resolved || BUILD_TIME_API_BASE;
  if (_remoteApiBase) {
    syncRelayApiBaseWithRemote(_remoteApiBase);
  }
}

/**
 * Get the current remote API base URL.
 * Returns the runtime value if set by ConfigProvider, otherwise the build-time default.
 */
export function getRemoteApiUrl(): string {
  return _remoteApiBase;
}

// Backward-compatible export — consumers should migrate to getRemoteApiUrl()
export const REMOTE_API_URL = BUILD_TIME_API_BASE;

export const makeRequest = async (
  path: string,
  options: RequestInit = {},
  retryOn401 = true
): Promise<Response> => {
  return makeAuthenticatedRequest(getRemoteApiUrl(), path, options, retryOn401);
};

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

  // Handle 401 - token may have expired
  if (response.status === 401 && retryOn401) {
    const newToken = await authRuntime.triggerRefresh();
    if (newToken) {
      // Retry the request with the new token
      headers.set('Authorization', `Bearer ${newToken}`);
      return fetch(`${baseUrl}${path}`, {
        ...options,
        headers,
        credentials: 'include',
      });
    }
    // Refresh failed, throw an auth error
    throw new Error('Session expired. Please log in again.');
  }

  return response;
}

export interface BulkUpdateIssueItem {
  id: string;
  changes: Partial<UpdateIssueRequest>;
}

export interface BulkUpdateProjectItem {
  id: string;
  changes: Partial<UpdateProjectRequest>;
}

export async function bulkUpdateProjects(
  updates: BulkUpdateProjectItem[]
): Promise<void> {
  const response = await makeRequest('/v1/projects/bulk', {
    method: 'POST',
    body: JSON.stringify({
      updates: updates.map((u) => ({ id: u.id, ...u.changes })),
    }),
  });
  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.message || 'Failed to bulk update projects');
  }
}

export async function bulkUpdateIssues(
  updates: BulkUpdateIssueItem[]
): Promise<void> {
  const response = await makeRequest('/v1/issues/bulk', {
    method: 'POST',
    body: JSON.stringify({
      updates: updates.map((u) => ({ id: u.id, ...u.changes })),
    }),
  });
  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.message || 'Failed to bulk update issues');
  }
}

export interface BulkUpdateProjectStatusItem {
  id: string;
  changes: Partial<UpdateProjectStatusRequest>;
}

export async function bulkUpdateProjectStatuses(
  updates: BulkUpdateProjectStatusItem[]
): Promise<void> {
  const response = await makeRequest('/v1/project_statuses/bulk', {
    method: 'POST',
    body: JSON.stringify({
      updates: updates.map((u) => ({ id: u.id, ...u.changes })),
    }),
  });
  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.message || 'Failed to bulk update project statuses');
  }
}

// ---------------------------------------------------------------------------
// Relay host API functions (served by remote backend)
// ---------------------------------------------------------------------------

export async function listRelayHosts(): Promise<RelayHost[]> {
  const response = await makeRequest('/v1/hosts', { method: 'GET' });
  if (!response.ok) {
    throw await parseErrorResponse(response, 'Failed to list relay hosts');
  }

  const body = (await response.json()) as ListRelayHostsResponse;
  return body.hosts;
}

// ---------------------------------------------------------------------------
// SAS URL cache with TTL — SAS URLs expire after 5 minutes, cache for 4
// ---------------------------------------------------------------------------

const SAS_URL_TTL_MS = 4 * 60 * 1000;

interface CachedSasUrl {
  url: string;
  expiresAt: number;
}

const sasUrlCache = new Map<string, CachedSasUrl>();

// ---------------------------------------------------------------------------
// Utility: SHA-256 file hash
// ---------------------------------------------------------------------------

export async function computeFileHash(file: File): Promise<string> {
  const buffer = await file.arrayBuffer();
  const hash = sha256Bytes(new Uint8Array(buffer));
  return Array.from(hash)
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}

// ---------------------------------------------------------------------------
// Utility: Upload to Azure Blob Storage with progress
// ---------------------------------------------------------------------------

export function uploadToAzure(
  uploadUrl: string,
  file: File,
  onProgress?: (pct: number) => void
): Promise<void> {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    xhr.open('PUT', uploadUrl, true);
    xhr.setRequestHeader('x-ms-blob-type', 'BlockBlob');
    xhr.setRequestHeader('Content-Type', file.type);

    if (onProgress) {
      xhr.upload.addEventListener('progress', (e) => {
        if (e.lengthComputable) {
          onProgress(Math.round((e.loaded / e.total) * 100));
        }
      });
    }

    xhr.onload = () => {
      if (xhr.status === 201) {
        resolve();
      } else {
        reject(
          new Error(
            `Azure upload failed with status ${xhr.status}: ${xhr.statusText}`
          )
        );
      }
    };

    xhr.onerror = () => {
      reject(new Error('Azure upload failed: network error'));
    };

    xhr.send(file);
  });
}

// ---------------------------------------------------------------------------
// Utility: safe error response parsing (handles non-JSON error bodies)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Attachment API functions
// ---------------------------------------------------------------------------

export async function initAttachmentUpload(
  params: InitUploadRequest
): Promise<InitUploadResponse> {
  const response = await makeRequest('/v1/attachments/init', {
    method: 'POST',
    body: JSON.stringify(params),
  });
  if (!response.ok) {
    throw await parseErrorResponse(
      response,
      'Failed to init attachment upload'
    );
  }
  return response.json();
}

export async function confirmAttachmentUpload(
  params: ConfirmUploadRequest
): Promise<AttachmentWithBlob> {
  const response = await makeRequest('/v1/attachments/confirm', {
    method: 'POST',
    body: JSON.stringify(params),
  });
  if (!response.ok) {
    throw await parseErrorResponse(
      response,
      'Failed to confirm attachment upload'
    );
  }
  return response.json();
}

export async function commitIssueAttachments(
  issueId: string,
  request: CommitAttachmentsRequest
): Promise<CommitAttachmentsResponse> {
  const response = await makeRequest(
    `/v1/issues/${issueId}/attachments/commit`,
    {
      method: 'POST',
      body: JSON.stringify(request),
    }
  );
  if (!response.ok) {
    throw await parseErrorResponse(
      response,
      'Failed to commit issue attachments'
    );
  }
  return response.json();
}

export async function commitCommentAttachments(
  commentId: string,
  request: CommitAttachmentsRequest
): Promise<CommitAttachmentsResponse> {
  const response = await makeRequest(
    `/v1/comments/${commentId}/attachments/commit`,
    {
      method: 'POST',
      body: JSON.stringify(request),
    }
  );
  if (!response.ok) {
    throw await parseErrorResponse(
      response,
      'Failed to commit comment attachments'
    );
  }
  return response.json();
}

export async function deleteAttachment(attachmentId: string): Promise<void> {
  const response = await makeRequest(`/v1/attachments/${attachmentId}`, {
    method: 'DELETE',
  });
  if (!response.ok) {
    throw await parseErrorResponse(response, 'Failed to delete attachment');
  }
}

export async function fetchAttachmentSasUrl(
  attachmentId: string,
  type: 'file' | 'thumbnail'
): Promise<string> {
  const cacheKey = `${attachmentId}:${type}`;
  const cached = sasUrlCache.get(cacheKey);
  if (cached && Date.now() < cached.expiresAt) {
    return cached.url;
  }

  const response = await makeRequest(`/v1/attachments/${attachmentId}/${type}`);
  if (!response.ok) {
    throw new Error(
      `Failed to fetch attachment ${type}: ${response.statusText}`
    );
  }

  const data: AttachmentUrlResponse = await response.json();
  sasUrlCache.set(cacheKey, {
    url: data.url,
    expiresAt: Date.now() + SAS_URL_TTL_MS,
  });
  return data.url;
}
