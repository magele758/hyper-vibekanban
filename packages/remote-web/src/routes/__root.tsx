import { type ReactNode, useMemo, useRef } from "react";
import {
  createRootRoute,
  Outlet,
  useLocation,
  useParams,
} from "@tanstack/react-router";
import { Provider as NiceModalProvider } from "@ebay/nice-modal-react";
import { useSystemTheme } from "@remote/shared/hooks/useSystemTheme";
import { RemoteActionsProvider } from "@remote/app/providers/RemoteActionsProvider";
import { RemoteUserSystemProvider } from "@remote/app/providers/RemoteUserSystemProvider";
import { RemoteAppShell } from "@remote/app/layout/RemoteAppShell";
import { UserProvider } from "@/shared/providers/remote/UserProvider";
import { WorkspaceProvider } from "@/shared/providers/WorkspaceProvider";
import { ExecutionProcessesProvider } from "@/shared/providers/ExecutionProcessesProvider";
import { TerminalProvider } from "@/shared/providers/TerminalProvider";
import { LogsPanelProvider } from "@/shared/providers/LogsPanelProvider";
import { ActionsProvider } from "@/shared/providers/ActionsProvider";
import { useAuth } from "@/shared/hooks/auth/useAuth";
import { useKanbanIssueComposerScratch } from "@/shared/hooks/useKanbanIssueComposerScratch";
import { useUiPreferencesScratch } from "@/shared/hooks/useUiPreferencesScratch";
import { useWorkspaceContext } from "@/shared/hooks/useWorkspaceContext";
import { AppNavigationProvider } from "@/shared/hooks/useAppNavigation";
import {
  SequenceTrackerProvider,
  SequenceIndicator,
  useWorkspaceShortcuts,
  useIssueShortcuts,
  useKeyShowHelp,
  Scope,
} from "@/shared/keyboard";
import { KeyboardShortcutsDialog } from "@/shared/dialogs/shared/KeyboardShortcutsDialog";
import {
  createRemoteHostAppNavigation,
  remoteFallbackAppNavigation,
  resolveRemoteDestinationFromPath,
} from "@remote/app/navigation/AppNavigation";
import {
  resolveRelayNavigationHostId,
  useRelayAppBarHosts,
} from "@remote/shared/hooks/useRelayAppBarHosts";
import { setActiveRelayHostId } from "@remote/shared/lib/relay/activeHostContext";
import { setRelayHostFallback } from "@/shared/lib/relayHostFallback";
import { HostIdProvider } from "@/shared/providers/HostIdProvider";
import {
  isProjectDestination,
  isWorkspacesDestination,
} from "@/shared/lib/routes/appNavigation";
import NotFoundPage from "../pages/NotFoundPage";

export const Route = createRootRoute({
  component: RootLayout,
  notFoundComponent: NotFoundPage,
});

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

/**
 * Global keyboard shortcut that doesn't require workspace/actions context.
 * Renders inside HotkeysProvider (from App.tsx) but outside WorkspaceProvider.
 */
function GlobalKeyboardShortcuts() {
  useKeyShowHelp(
    () => {
      KeyboardShortcutsDialog.show();
    },
    { scope: Scope.GLOBAL },
  );
  return null;
}

/**
 * Workspace & issue keyboard shortcuts that require ActionsProvider + WorkspaceProvider.
 * Must be rendered inside WorkspaceRouteProviders.
 */
function WorkspaceKeyboardShortcuts() {
  useWorkspaceShortcuts();
  useIssueShortcuts();
  return null;
}

function WorkspaceRouteProviders({ children }: { children: ReactNode }) {
  return (
    <WorkspaceProvider>
      <ExecutionProcessesProviderWrapper>
        <TerminalProvider>
          <LogsPanelProvider>
            <ActionsProvider>
              <WorkspaceKeyboardShortcuts />
              {children}
            </ActionsProvider>
          </LogsPanelProvider>
        </TerminalProvider>
      </ExecutionProcessesProviderWrapper>
    </WorkspaceProvider>
  );
}

function RootLayout() {
  useSystemTheme();
  const { isSignedIn } = useAuth();
  const { hostId } = useParams({ strict: false });
  const routeHostId = hostId ?? null;
  const { hosts: relayHosts, isLoading: relayHostsLoading } =
    useRelayAppBarHosts(isSignedIn);
  const lastNavigationHostIdRef = useRef<string | null>(null);
  const navigationHostId = useMemo(() => {
    const resolved = resolveRelayNavigationHostId(relayHosts, {
      routeHostId,
    });
    if (resolved) {
      lastNavigationHostIdRef.current = resolved;
      return resolved;
    }

    if (relayHostsLoading && lastNavigationHostIdRef.current) {
      return lastNavigationHostIdRef.current;
    }

    return null;
  }, [relayHosts, routeHostId, relayHostsLoading]);

  // Sync before child effects so relay/local API transport can resolve hostId.
  setActiveRelayHostId(navigationHostId);
  setRelayHostFallback(navigationHostId);

  const appNavigation = useMemo(
    () =>
      navigationHostId
        ? createRemoteHostAppNavigation(navigationHostId)
        : remoteFallbackAppNavigation,
    [navigationHostId],
  );

  return (
    <AppNavigationProvider value={appNavigation}>
      <HostIdProvider>
        <RootLayoutContent
          key={routeHostId ?? navigationHostId ?? "remote-default"}
        />
      </HostIdProvider>
    </AppNavigationProvider>
  );
}

function RootLayoutContent() {
  useUiPreferencesScratch();
  useKanbanIssueComposerScratch();
  const location = useLocation();
  const isStandaloneRoute =
    location.pathname.startsWith("/account") ||
    location.pathname.startsWith("/login") ||
    location.pathname.startsWith("/invitations");
  const destination = resolveRemoteDestinationFromPath(location.pathname);
  const isWorkspaceProviderRoute =
    isProjectDestination(destination) || isWorkspacesDestination(destination);

  const pageContent = isStandaloneRoute ? (
    <Outlet />
  ) : (
    <SequenceTrackerProvider>
      <SequenceIndicator />
      <GlobalKeyboardShortcuts />
      <RemoteAppShell>
        <Outlet />
      </RemoteAppShell>
    </SequenceTrackerProvider>
  );

  const content = isWorkspaceProviderRoute ? (
    <WorkspaceRouteProviders>
      <NiceModalProvider>{pageContent}</NiceModalProvider>
    </WorkspaceRouteProviders>
  ) : (
    <NiceModalProvider>{pageContent}</NiceModalProvider>
  );

  return (
    <UserProvider>
      <RemoteActionsProvider>
        <RemoteUserSystemProvider>{content}</RemoteUserSystemProvider>
      </RemoteActionsProvider>
    </UserProvider>
  );
}
