import { useCallback, useLayoutEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { useAppRuntime } from '@/shared/hooks/useAppRuntime';
import { useCurrentKanbanRouteState } from '@/shared/hooks/useCurrentKanbanRouteState';
import { useExecutionHostId } from '@/shared/hooks/useExecutionHostId';
import { useRemoteCloudHostsAppBarModel } from '@/shared/hooks/useRemoteCloudHosts';
import { useCreateMode } from '@/features/create-mode/model/useCreateMode';
import { cn } from '@/shared/lib/utils';

const LOCAL_HOST_VALUE = 'local';

/**
 * Desktop-only host picker for workspace create.
 * Selecting a paired host updates executionHostId and re-routes create so
 * HostIdProvider / localApiTransport target /api/host/{id}/...
 */
export function CreateModeHostPicker() {
  const { t } = useTranslation('common');
  const runtime = useAppRuntime();
  const appNavigation = useAppNavigation();
  const routeState = useCurrentKanbanRouteState();
  const { clearRepos } = useCreateMode();
  const { hosts: remoteHosts } = useRemoteCloudHostsAppBarModel();
  const { executionHostId, setExecutionHostId } = useExecutionHostId();

  // Seed from the create URL when it carries a host (deep link / AppBar).
  // Depend only on routeState.hostId — including executionHostId would re-run
  // after setExecutionHostId(null) while the URL still has /hosts/{id}/... and
  // bounce the picker back to the remote host mid-switch.
  useLayoutEffect(() => {
    if (runtime !== 'local' || !routeState.hostId) {
      return;
    }
    setExecutionHostId(routeState.hostId);
  }, [routeState.hostId, runtime, setExecutionHostId]);

  const selectedValue = executionHostId ?? LOCAL_HOST_VALUE;

  const selectedRemoteHost = useMemo(
    () =>
      executionHostId
        ? (remoteHosts.find((host) => host.id === executionHostId) ?? null)
        : null,
    [executionHostId, remoteHosts]
  );

  const navigateCreateForHost = useCallback(
    (hostId: string | null) => {
      const { projectId, issueId, draftId } = routeState;

      // Pass hostId explicitly so navigation doesn't race AppBar sync reading
      // a stale URL /hosts/{id}/... segment before the route updates.
      if (projectId && issueId && draftId) {
        appNavigation.goToProjectIssueWorkspaceCreate(
          projectId,
          issueId,
          draftId,
          { replace: true },
          hostId
        );
        return;
      }

      if (projectId && draftId) {
        appNavigation.goToProjectWorkspaceCreate(
          projectId,
          draftId,
          { replace: true },
          hostId
        );
        return;
      }

      setExecutionHostId(hostId);
      appNavigation.goToWorkspacesCreate({ replace: true }, hostId);
    },
    [appNavigation, routeState, setExecutionHostId]
  );

  const handleChange = useCallback(
    async (nextValue: string) => {
      const nextHostId = nextValue === LOCAL_HOST_VALUE ? null : nextValue;
      if (nextHostId === executionHostId) {
        return;
      }

      if (nextHostId) {
        const host = remoteHosts.find((item) => item.id === nextHostId);
        if (!host || host.status === 'offline') {
          return;
        }
      }

      // Flush empty repos before remount so the new host doesn't restore
      // the previous machine's repo IDs from draft scratch.
      await clearRepos();
      // Set + navigate with the same hostId so URL and executionHostId stay
      // aligned. Create routes are not overwritten by SharedAppLayout sync.
      setExecutionHostId(nextHostId);
      navigateCreateForHost(nextHostId);
    },
    [
      clearRepos,
      executionHostId,
      navigateCreateForHost,
      remoteHosts,
      setExecutionHostId,
    ]
  );

  if (runtime !== 'local') {
    return null;
  }

  if (remoteHosts.length === 0) {
    return null;
  }

  const offlineSelected = selectedRemoteHost?.status === 'offline';

  return (
    <div className="mb-base flex flex-col items-center gap-half">
      <label className="flex items-center gap-half text-sm text-mid">
        <span>
          {t('createMode.hostPicker.label', {
            defaultValue: 'Run on',
          })}
        </span>
        <select
          className={cn(
            'rounded border border-low bg-transparent px-1 py-0.5 text-sm text-high',
            offlineSelected && 'opacity-50'
          )}
          value={selectedValue}
          onChange={(event) => handleChange(event.target.value)}
        >
          <option value={LOCAL_HOST_VALUE}>
            {t('createMode.hostPicker.thisMachine', {
              defaultValue: 'This machine',
            })}
          </option>
          {remoteHosts.map((host) => {
            const offline = host.status === 'offline';
            return (
              <option key={host.id} value={host.id} disabled={offline}>
                {offline
                  ? t('createMode.hostPicker.offlineHost', {
                      defaultValue: '{{name}} (offline)',
                      name: host.name,
                    })
                  : host.name}
              </option>
            );
          })}
        </select>
      </label>
      {offlineSelected && (
        <p className="text-center text-xs text-error">
          {t('createMode.hostPicker.offlineHint', {
            defaultValue:
              'Selected host is offline. Choose this machine or another online host.',
          })}
        </p>
      )}
    </div>
  );
}
