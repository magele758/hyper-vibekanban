import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import {
  LOCAL_CREATE_HOST_VALUE,
  createHostIdFromPickerValue,
  isCreateHostRunnable,
  resolveCreateModeHostValue,
  showsThisMachineOption,
} from './createModeHostPickerModel.ts';

const hosts = [
  { id: 'offline-a', name: 'A', status: 'offline' },
  { id: 'online-b', name: 'B', status: 'online' },
];

describe('create mode host picker model', () => {
  it('hides this machine on the data service page', () => {
    assert.equal(showsThisMachineOption('remote'), false);
    assert.equal(showsThisMachineOption('local'), true);
  });

  it('does not treat an unknown remote route host as this machine', () => {
    assert.equal(
      resolveCreateModeHostValue({
        runtime: 'remote',
        executionHostId: null,
        routeHostId: 'missing',
        hosts,
      }),
      ''
    );
    assert.equal(
      resolveCreateModeHostValue({
        runtime: 'remote',
        executionHostId: null,
        routeHostId: 'online-b',
        hosts,
      }),
      'online-b'
    );
  });

  it('keeps the desktop default on this machine', () => {
    assert.equal(
      resolveCreateModeHostValue({
        runtime: 'local',
        executionHostId: null,
        routeHostId: null,
        hosts,
      }),
      LOCAL_CREATE_HOST_VALUE
    );
  });

  it('rejects this machine on the data service page', () => {
    assert.equal(
      createHostIdFromPickerValue('remote', LOCAL_CREATE_HOST_VALUE),
      undefined
    );
    assert.equal(createHostIdFromPickerValue('remote', ''), undefined);
    assert.equal(createHostIdFromPickerValue('remote', 'online-b'), 'online-b');
    assert.equal(
      createHostIdFromPickerValue('local', LOCAL_CREATE_HOST_VALUE),
      null
    );
  });

  it('requires an online host before a data-service create can run', () => {
    assert.equal(
      isCreateHostRunnable({
        runtime: 'remote',
        hostId: null,
        hosts,
      }),
      false
    );
    assert.equal(
      isCreateHostRunnable({
        runtime: 'remote',
        hostId: 'offline-a',
        hosts,
      }),
      false
    );
    assert.equal(
      isCreateHostRunnable({
        runtime: 'remote',
        hostId: 'online-b',
        hosts,
      }),
      true
    );
    assert.equal(
      isCreateHostRunnable({
        runtime: 'local',
        hostId: null,
        hosts,
      }),
      true
    );
  });
});
