import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { workspaceExecutionRequestOptions } from './workspaceExecutionTarget.ts';

describe('workspaceExecutionRequestOptions', () => {
  it('relays a data-service create to the selected host', () => {
    assert.deepEqual(
      workspaceExecutionRequestOptions({
        runtime: 'remote',
        hostId: 'host-b',
      }),
      {
        hostScope: 'none',
        relayHostId: 'host-b',
      }
    );
  });

  it('refuses to post a data-service create without a host', () => {
    assert.throws(
      () =>
        workspaceExecutionRequestOptions({
          runtime: 'remote',
          hostId: null,
        }),
      /online host/
    );
  });

  it('keeps desktop this-machine creates on the local API', () => {
    assert.deepEqual(
      workspaceExecutionRequestOptions({
        runtime: 'local',
        hostId: null,
      }),
      { hostScope: 'none' }
    );
  });

  it('prefixes desktop creates that target a paired host', () => {
    assert.deepEqual(
      workspaceExecutionRequestOptions({
        runtime: 'local',
        hostId: 'host-b',
      }),
      {
        hostScope: 'explicit',
        hostId: 'host-b',
      }
    );
  });
});
