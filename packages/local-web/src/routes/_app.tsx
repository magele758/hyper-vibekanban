import { useEffect, type ReactNode } from 'react';
import { createFileRoute, useLocation } from '@tanstack/react-router';
import { Provider as NiceModalProvider } from '@ebay/nice-modal-react';
import { SequenceTrackerProvider } from '@/shared/keyboard/SequenceTracker';
import { SequenceIndicator } from '@/shared/keyboard/SequenceIndicator';
import { useWorkspaceShortcuts } from '@/shared/keyboard/useWorkspaceShortcuts';
import { useIssueShortcuts } from '@/shared/keyboard/useIssueShortcuts';
import { useKeyShowHelp, Scope } from '@/shared/keyboard';
import { KeyboardShortcutsDialog } from '@/shared/dialogs/shared/KeyboardShortcutsDialog';
import { ReleaseNotesDialog } from '@/shared/dialogs/global/ReleaseNotesDialog';
import { TerminalProvider } from '@/shared/providers/TerminalProvider';
import { HostIdProvider } from '@/shared/providers/HostIdProvider';
import { setRelayHostFallback } from '@/shared/lib/relayHostFallback';
import { useCurrentAppDestination } from '@/shared/hooks/useCurrentAppDestination';
import { useExecutionHostId } from '@/shared/hooks/useExecutionHostId';
import {
  getDestinationHostId,
  isLocalWorkspacesDestination,
} from '@/shared/lib/routes/appNavigation';
import { WorkspaceProvider } from '@/shared/providers/WorkspaceProvider';
import { ExecutionProcessesProvider } from '@/shared/providers/ExecutionProcessesProvider';
import { LogsPanelProvider } from '@/shared/providers/LogsPanelProvider';
import { ActionsProvider } from '@/shared/providers/ActionsProvider';
import { useWorkspaceContext } from '@/shared/hooks/useWorkspaceContext';
import { useUserSystem } from '@/shared/hooks/useUserSystem';
import { SharedAppLayout } from '@/shared/components/ui-new/containers/SharedAppLayout';

function KeyboardShortcutsHandler() {
  useKeyShowHelp(
    () => {
      KeyboardShortcutsDialog.show();
    },
    { scope: Scope.GLOBAL }
  );
  useWorkspaceShortcuts();
  useIssueShortcuts();
  return null;
}

function ReleaseNotesHandler() {
  const { config, updateAndSaveConfig } = useUserSystem();
  const location = useLocation();

  useEffect(() => {
    if (!config || !config.remote_onboarding_acknowledged) return;

    const pathname = location.pathname;
    if (pathname.startsWith('/onboarding')) {
      return;
    }

    let cancelled = false;

    const showReleaseNotes = async () => {
      if (config.show_release_notes) {
        await ReleaseNotesDialog.show();
        if (!cancelled) {
          await updateAndSaveConfig({ show_release_notes: false });
        }
        ReleaseNotesDialog.hide();
      }
    };

    void showReleaseNotes();

    return () => {
      cancelled = true;
    };
  }, [config, updateAndSaveConfig, location.pathname]);

  return null;
}

function ExecutionProcessesProviderWrapper({
  children,
}: {
  children: ReactNode;
}) {
  const { selectedSessionId } = useWorkspaceContext();

  return (
    <ExecutionProcessesProvider sessionId={selectedSessionId}>
      {children}
    </ExecutionProcessesProvider>
  );
}

function AppRouteProviders({ children }: { children: ReactNode }) {
  return (
    <HostIdProvider>
      <WorkspaceProvider>
        <ExecutionProcessesProviderWrapper>
          <LogsPanelProvider>
            <ActionsProvider>
              {/* NiceModal renders dialogs as siblings of children at the
                  Provider level, so it must be inside all providers that
                  dialogs depend on (Workspace, Actions, etc.). */}
              <NiceModalProvider>{children}</NiceModalProvider>
            </ActionsProvider>
          </LogsPanelProvider>
        </ExecutionProcessesProviderWrapper>
      </WorkspaceProvider>
    </HostIdProvider>
  );
}

function AppLayoutRouteComponent() {
  const destination = useCurrentAppDestination();
  const { executionHostId } = useExecutionHostId();
  // Resolve host from the destination, not useParams — stale hostId params
  // after leaving /hosts/{id}/... would remount providers under the wrong key.
  const destinationHostId = getDestinationHostId(destination);
  const providerHostKey = destinationHostId ?? 'local';

  // Desktop: routes without an explicit /hosts/{id} segment (e.g. kanban
  // boards) inherit the selected execution host so HostIdProvider + the local
  // API transport target /api/host/{id}/... (mirrors remote-web's
  // setRelayHostFallback). Force local on explicit local workspaces/create
  // routes so this-machine flows are never proxied to a remote host.
  const isLocalCreateDestination =
    destination?.kind === 'workspaces-create' ||
    destination?.kind === 'project-workspace-create' ||
    destination?.kind === 'project-issue-workspace-create';
  const fallbackHostId =
    isLocalWorkspacesDestination(destination) ||
    (isLocalCreateDestination && !destinationHostId)
      ? null
      : executionHostId;
  setRelayHostFallback(fallbackHostId);

  return (
    <AppRouteProviders key={providerHostKey}>
      <ReleaseNotesHandler />
      <SequenceTrackerProvider>
        <SequenceIndicator />
        <KeyboardShortcutsHandler />
        <TerminalProvider>
          <SharedAppLayout />
        </TerminalProvider>
      </SequenceTrackerProvider>
    </AppRouteProviders>
  );
}

export const Route = createFileRoute('/_app')({
  component: AppLayoutRouteComponent,
});
