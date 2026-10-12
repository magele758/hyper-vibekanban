import { describe, expect, it } from 'vitest';
import {
  issueCreateNavigationHostId,
  resolveIssueWorkspaceCreateHostId,
} from './issueWorkspaceCreateHost';

const hosts = [
  { id: 'offline-first', status: 'offline' },
  { id: 'online-b', status: 'online' },
  { id: 'online-c', status: 'online' },
];

describe('resolveIssueWorkspaceCreateHostId', () => {
  it('uses the selected online host instead of the first machine', () => {
    expect(
      resolveIssueWorkspaceCreateHostId({
        runtime: 'remote',
        routeHostId: null,
        executionHostId: 'online-c',
        pageHostId: 'offline-first',
        hosts,
      })
    ).toBe('online-c');
  });

  it('prefers the route host when it is online', () => {
    expect(
      resolveIssueWorkspaceCreateHostId({
        runtime: 'local',
        routeHostId: 'online-b',
        executionHostId: 'online-c',
        pageHostId: 'offline-first',
        hosts,
      })
    ).toBe('online-b');
  });

  it('does not replace an offline desktop selection with hosts[0]', () => {
    expect(
      resolveIssueWorkspaceCreateHostId({
        runtime: 'local',
        executionHostId: 'offline-first',
        pageHostId: 'offline-first',
        hosts,
      })
    ).toBeNull();
  });

  it('uses the only other online host when the page host is offline', () => {
    expect(
      resolveIssueWorkspaceCreateHostId({
        runtime: 'remote',
        pageHostId: 'offline-first',
        hosts: hosts.slice(0, 2),
      })
    ).toBe('online-b');
  });

  it('does not guess when several hosts are online and none is selected', () => {
    expect(
      resolveIssueWorkspaceCreateHostId({
        runtime: 'remote',
        pageHostId: 'offline-first',
        hosts,
      })
    ).toBeNull();
  });

  it('keeps an explicit id when host status has not loaded', () => {
    expect(
      resolveIssueWorkspaceCreateHostId({
        runtime: 'remote',
        executionHostId: 'online-c',
        hosts: [],
      })
    ).toBe('online-c');
  });
});

describe('issueCreateNavigationHostId', () => {
  it('passes the chosen host through', () => {
    expect(
      issueCreateNavigationHostId({
        runtime: 'remote',
        targetHostId: 'online-c',
        pageHostId: 'offline-first',
        hosts,
      })
    ).toBe('online-c');
  });

  it('forces this machine on desktop when no online host is selected', () => {
    expect(
      issueCreateNavigationHostId({
        runtime: 'local',
        targetHostId: null,
        pageHostId: 'offline-first',
        hosts,
      })
    ).toBeNull();
  });

  it('refuses to inherit an offline remote page host', () => {
    expect(
      issueCreateNavigationHostId({
        runtime: 'remote',
        targetHostId: null,
        pageHostId: 'offline-first',
        hosts,
      })
    ).toBeNull();
  });
});
