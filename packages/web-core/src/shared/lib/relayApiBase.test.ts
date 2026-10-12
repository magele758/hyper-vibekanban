import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { resolveRelayApiBaseForPage } from './relayApiBase.ts';

const TS_HOST = 'shinemo.taile7bb25.ts.net';
const RELAY_PORT = '18082';
const DOOR_PORT = '18443';

function resolve(input: {
  bakedRelayApiBase?: string;
  pageProtocol: string;
  pageHostname: string;
  relayPort?: string;
  tailscaleRelayHttpsPort?: string;
}): string {
  return resolveRelayApiBaseForPage({
    bakedRelayApiBase: input.bakedRelayApiBase ?? '',
    pageProtocol: input.pageProtocol,
    pageHostname: input.pageHostname,
    relayPort: input.relayPort ?? RELAY_PORT,
    tailscaleRelayHttpsPort: input.tailscaleRelayHttpsPort ?? DOOR_PORT,
  });
}

describe('resolveRelayApiBaseForPage', () => {
  it('uses the Tailscale relay door on an https ts.net page when nothing is baked', () => {
    assert.equal(
      resolve({
        pageProtocol: 'https:',
        pageHostname: TS_HOST,
      }),
      `https://${TS_HOST}:${DOOR_PORT}`
    );
  });

  it('does not keep https://ts.net:18082 when the baked relay base is http', () => {
    assert.equal(
      resolve({
        bakedRelayApiBase: 'http://localhost:18082',
        pageProtocol: 'https:',
        pageHostname: TS_HOST,
      }),
      `https://${TS_HOST}:${DOOR_PORT}`
    );
  });

  it('rewrites a same-host baked https relay URL that still uses 18082', () => {
    assert.equal(
      resolve({
        bakedRelayApiBase: `https://${TS_HOST}:18082`,
        pageProtocol: 'https:',
        pageHostname: TS_HOST,
      }),
      `https://${TS_HOST}:${DOOR_PORT}`
    );
  });

  it('keeps a baked https door URL on the same ts.net host', () => {
    assert.equal(
      resolve({
        bakedRelayApiBase: `https://${TS_HOST}:${DOOR_PORT}`,
        pageProtocol: 'https:',
        pageHostname: TS_HOST,
      }),
      `https://${TS_HOST}:${DOOR_PORT}`
    );
  });

  it('honors VITE_TAILSCALE_RELAY_HTTPS_PORT', () => {
    assert.equal(
      resolve({
        pageProtocol: 'https:',
        pageHostname: TS_HOST,
        tailscaleRelayHttpsPort: '18444',
      }),
      `https://${TS_HOST}:18444`
    );
  });

  it('keeps plaintext 18082 on an http ts.net page', () => {
    assert.equal(
      resolve({
        bakedRelayApiBase: 'http://localhost:18082',
        pageProtocol: 'http:',
        pageHostname: TS_HOST,
      }),
      `http://${TS_HOST}:${RELAY_PORT}`
    );
  });

  it('does not move a localhost https page onto 18443', () => {
    assert.equal(
      resolve({
        bakedRelayApiBase: 'https://localhost:13445',
        pageProtocol: 'https:',
        pageHostname: 'localhost',
      }),
      'https://localhost:13445'
    );
  });

  it('does not remap a remote relay onto a localhost worker desktop', () => {
    assert.equal(
      resolve({
        bakedRelayApiBase: `https://${TS_HOST}:${DOOR_PORT}`,
        pageProtocol: 'http:',
        pageHostname: 'localhost',
      }),
      `https://${TS_HOST}:${DOOR_PORT}`
    );
  });

  it('keeps the raw relay port for an https LAN page', () => {
    assert.equal(
      resolve({
        bakedRelayApiBase: 'http://localhost:18082',
        pageProtocol: 'https:',
        pageHostname: '192.168.1.20',
      }),
      'https://192.168.1.20:18082'
    );
  });
});
