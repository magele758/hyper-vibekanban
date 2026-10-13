import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { hostIsLinked, relayListStatus } from './relayHostStatus.ts';

describe('relayListStatus', () => {
  it('stays unpaired until a browser or the data service has the link', () => {
    assert.equal(relayListStatus('online', false), 'unpaired');
    assert.equal(relayListStatus('offline', false), 'unpaired');
  });

  it('follows the relay connection once the host is linked', () => {
    assert.equal(relayListStatus('online', true), 'online');
    assert.equal(relayListStatus('offline', true), 'offline');
  });
});

describe('hostIsLinked', () => {
  it('accepts either IndexedDB or a server-side host link', () => {
    const indexed = new Set(['local']);
    const server = new Set(['joined']);
    assert.equal(hostIsLinked('local', indexed, server), true);
    assert.equal(hostIsLinked('joined', indexed, server), true);
    assert.equal(hostIsLinked('other', indexed, server), false);
  });
});
