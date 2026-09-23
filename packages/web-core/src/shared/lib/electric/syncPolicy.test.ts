import { describe, expect, it } from 'vitest';
import {
  CONSTRAINED_SHAPE_READY_TIMEOUT_MS,
  isConstrainedElectricSyncFrom,
  isPlainHttpUrl,
  resolveElectricShapeSyncOptions,
} from './syncPolicy';

describe('isPlainHttpUrl', () => {
  it('treats http Electric bases as constrained', () => {
    expect(isPlainHttpUrl('http://100.x.y.z:13000')).toBe(true);
    expect(isPlainHttpUrl('http://localhost:13000')).toBe(true);
  });

  it('treats https front doors as multiplexed', () => {
    expect(isPlainHttpUrl('https://localhost:13443')).toBe(false);
    expect(isPlainHttpUrl('https://host.ts.net:13444')).toBe(false);
  });
});

describe('isConstrainedElectricSyncFrom', () => {
  it('is unconstrained on an https page even if the baked API is http', () => {
    expect(
      isConstrainedElectricSyncFrom('https:', 'http://127.0.0.1:13000')
    ).toBe(false);
  });

  it('is constrained on an http page talking to an http API', () => {
    expect(
      isConstrainedElectricSyncFrom('http:', 'http://100.1.2.3:13000')
    ).toBe(true);
  });

  it('is unconstrained when the API itself is https', () => {
    expect(
      isConstrainedElectricSyncFrom('http:', 'https://localhost:13443')
    ).toBe(false);
  });

  it('follows the page protocol when the API base is empty', () => {
    expect(isConstrainedElectricSyncFrom('http:', '')).toBe(true);
    expect(isConstrainedElectricSyncFrom('https:', '')).toBe(false);
  });
});

describe('resolveElectricShapeSyncOptions', () => {
  it('keeps live subscribe on unconstrained connections', () => {
    expect(
      resolveElectricShapeSyncOptions({
        constrained: false,
        lane: 'snapshot',
        unconstrainedReadyTimeoutMs: 5_000,
      })
    ).toEqual({ subscribe: true, readyTimeoutMs: 5_000 });
  });

  it('keeps live shapes subscribed on HTTP/1.1', () => {
    expect(
      resolveElectricShapeSyncOptions({
        constrained: true,
        lane: 'live',
        unconstrainedReadyTimeoutMs: 5_000,
      })
    ).toEqual({
      subscribe: true,
      readyTimeoutMs: CONSTRAINED_SHAPE_READY_TIMEOUT_MS,
    });
  });

  it('snapshots secondary shapes on HTTP/1.1 so they release the slot', () => {
    expect(
      resolveElectricShapeSyncOptions({
        constrained: true,
        lane: 'snapshot',
      })
    ).toEqual({
      subscribe: false,
      readyTimeoutMs: CONSTRAINED_SHAPE_READY_TIMEOUT_MS,
    });
  });
});
