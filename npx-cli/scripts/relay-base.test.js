'use strict';

const assert = require('node:assert/strict');
const { describe, it } = require('node:test');
const {
  normalizeRelayApiBase,
  deriveRelayApiBase,
  resolveJoinRelayBase,
} = require('./relay-base');

const TS_HOST = 'shinemo.taile7bb25.ts.net';

describe('normalizeRelayApiBase', () => {
  it('moves https tailscale relay off plaintext 18082', () => {
    assert.equal(
      normalizeRelayApiBase(`https://${TS_HOST}:18082`),
      `https://${TS_HOST}:18443`,
    );
    assert.equal(
      normalizeRelayApiBase(`https://${TS_HOST}`),
      `https://${TS_HOST}:18443`,
    );
    assert.equal(
      normalizeRelayApiBase(`https://${TS_HOST}:443/`),
      `https://${TS_HOST}:18443`,
    );
  });

  it('keeps an explicit tls door and custom ports', () => {
    assert.equal(
      normalizeRelayApiBase(`https://${TS_HOST}:18443/`),
      `https://${TS_HOST}:18443`,
    );
    assert.equal(
      normalizeRelayApiBase(`https://${TS_HOST}:18444`),
      `https://${TS_HOST}:18444`,
    );
  });

  it('does not rewrite plaintext or non-tailscale hosts', () => {
    assert.equal(
      normalizeRelayApiBase(`http://${TS_HOST}:18082`),
      `http://${TS_HOST}:18082`,
    );
    assert.equal(
      normalizeRelayApiBase('https://relay.example.com:18082'),
      'https://relay.example.com:18082',
    );
  });
});

describe('resolveJoinRelayBase', () => {
  it('derives the tls door from an https tailscale data service', () => {
    assert.equal(
      resolveJoinRelayBase(`https://${TS_HOST}:13444`, undefined),
      `https://${TS_HOST}:18443`,
    );
  });

  it('rewrites an explicit https 18082 relay base', () => {
    assert.equal(
      resolveJoinRelayBase(
        'http://127.0.0.1:13000',
        `https://${TS_HOST}:18082`,
      ),
      `https://${TS_HOST}:18443`,
    );
  });

  it('derives plaintext 18082 for an http data service', () => {
    assert.equal(
      deriveRelayApiBase('http://127.0.0.1:13000'),
      'http://127.0.0.1:18082',
    );
  });
});
