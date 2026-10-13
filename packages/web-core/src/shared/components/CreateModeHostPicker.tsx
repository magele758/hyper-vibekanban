import { useCallback, useLayoutEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { useAppRuntime } from '@/shared/hooks/useAppRuntime';
import { useCreateModeHosts } from '@/shared/hooks/useCreateModeHosts';
import { useCurrentAppDestination } from '@/shared/hooks/useCurrentAppDestination';
import { useCurrentKanbanRouteState } from '@/shared/hooks/useCurrentKanbanRouteState';
import { useExecutionHostId } from '@/shared/hooks/useExecutionHostId';
import { useCreateMode } from '@/features/create-mode/model/useCreateMode';
import {
  LOCAL_CREATE_HOST_VALUE,
  createHostIdFromPickerValue,
  resolveCreateModeHostValue,
  showsThisMachineOption,
} from '@/shared/lib/createModeHostPickerModel';
import { getDestinationHostId } from '@/shared/lib/routes/appNavigation';
import { cn } from '@/shared/lib/utils';

/**
 * Host picker for workspace create.
 * Desktop can run on this machine or a paired host.
 * The data service page only lists paired hosts and routes create to the
 * selected one (HostIdProvider / relay), never the data service process.
 */
export function CreateModeHostPicker() {
  const { t } = useTranslation('common');
  const runtime = useAppRuntime();
  const appNavigation = useAppNavigation();
  const destination = useCurrentAppDestination();
  const routeHostId = getDestinationHostId(destination);
  const routeState = useCurrentKanbanRouteState();
  const { clearRepos } = useCreateMode();
  const { hosts, isLoading } = useCreateModeHosts();
  const { executionHostId, setExecutionHostId } = useExecutionHostId();
  const includeThisMachine = showsThisMachineOption(runtime);

  // Seed from the create URL when it carries a host (deep link / AppBar).
  // Depend only on the route host — including executionHostId would re-run
  // after setExecutionHostId(null) while the URL still has /hosts/{id}/... and
  // bounce the picker back to the remote host mid-switch.
  useLayoutEffect(() => {
    if (!routeHostId) {
      return;
    }
    setExecutionHostId(routeHostId);
  }, [routeHostId, setExecutionHostId]);

  const selectedValue = resolveCreateModeHostValue({
    runtime,
    executionHostId,
    routeHostId,
    hosts,
  });

  const selectedRemoteHost = useMemo(
    () =>
      selectedValue && selectedValue !== LOCAL_CREATE_HOST_VALUE
        ? (hosts.find((host) => host.id === selectedValue) ?? null)
        : null,
    [hosts, selectedValue]
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
      const nextHostId = createHostIdFromPickerValue(runtime, nextValue);
      if (nextHostId === undefined || nextHostId === executionHostId) {
        return;
      }

      if (nextHostId) {
        const host = hosts.find((item) => item.id === nextHostId);
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
      hosts,
      navigateCreateForHost,
      runtime,
      setExecutionHostId,
    ]
  );

  if (runtime === 'local' && hosts.length === 0) {
    return null;
  }

  if (runtime === 'remote' && isLoading) {
    return null;
  }

  const offlineSelected = selectedRemoteHost?.status === 'offline';
  const needsHost = runtime === 'remote' && !selectedRemoteHost;

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
            (offlineSelected || needsHost) && 'opacity-50'
          )}
          value={selectedValue}
          onChange={(event) => handleChange(event.target.value)}
        >
          {includeThisMachine && (
            <option value={LOCAL_CREATE_HOST_VALUE}>
              {t('createMode.hostPicker.thisMachine', {
                defaultValue: 'This machine',
              })}
            </option>
          )}
          {runtime === 'remote' && !selectedValue && (
            <option value="" disabled>
              {t('createMode.hostPicker.selectHost', {
                defaultValue: 'Select a machine',
              })}
            </option>
          )}
          {hosts.map((host) => {
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
      {runtime === 'remote' && hosts.length === 0 && (
        <p className="text-center text-xs text-error">
          {t('createMode.hostPicker.noPairedHosts', {
            defaultValue:
              'No paired machines yet. Pair a host before creating a workspace.',
          })}
        </p>
      )}
      {offlineSelected && (
        <p className="text-center text-xs text-error">
          {t(
            runtime === 'remote'
              ? 'createMode.hostPicker.offlineHintRemote'
              : 'createMode.hostPicker.offlineHint',
            {
              defaultValue:
                runtime === 'remote'
                  ? 'Selected host is offline. Choose an online host.'
                  : 'Selected host is offline. Choose this machine or another online host.',
            }
          )}
        </p>
      )}
      {needsHost && hosts.length > 0 && (
        <p className="text-center text-xs text-error">
          {t('createMode.hostPicker.noOnlineHost', {
            defaultValue: 'Choose an online host to run this workspace.',
          })}
        </p>
      )}
    </div>
  );
}
