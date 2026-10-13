'use strict';

const TLS_DOOR = '18443';
const PLAIN_PORT = '18082';

function isTsNetHost(hostname) {
  return String(hostname || '')
    .replace(/\.$/, '')
    .toLowerCase()
    .endsWith('.ts.net');
}

function stripTrailingSlash(value) {
  return String(value || '').trim().replace(/\/+$/, '');
}

function normalizeRelayApiBase(raw) {
  const trimmed = stripTrailingSlash(raw);
  if (!trimmed.toLowerCase().startsWith('https://')) {
    return trimmed;
  }

  let url;
  try {
    url = new URL(trimmed);
  } catch {
    return trimmed;
  }

  if (!isTsNetHost(url.hostname)) {
    return trimmed;
  }

  if (url.port === '' || url.port === '443' || url.port === PLAIN_PORT) {
    url.port = TLS_DOOR;
    return stripTrailingSlash(url.toString());
  }

  return trimmed;
}

function deriveRelayApiBase(apiBase) {
  const trimmed = stripTrailingSlash(apiBase);
  let url;
  try {
    url = new URL(trimmed);
  } catch {
    return trimmed;
  }

  url.pathname = '';
  url.search = '';
  url.hash = '';
  url.port =
    url.protocol === 'https:' && isTsNetHost(url.hostname)
      ? TLS_DOOR
      : PLAIN_PORT;
  return normalizeRelayApiBase(url.origin);
}

function resolveJoinRelayBase(apiBase, relayBase) {
  const explicit = typeof relayBase === 'string' ? relayBase.trim() : '';
  if (explicit) {
    return normalizeRelayApiBase(explicit);
  }
  return deriveRelayApiBase(apiBase);
}

module.exports = {
  normalizeRelayApiBase,
  deriveRelayApiBase,
  resolveJoinRelayBase,
};
