import { useMemo, useCallback, type ReactNode } from 'react';
import { useShape } from '@/shared/integrations/electric/hooks';
import { USER_WORKSPACES_SHAPE, USER_INBOX_SHAPE } from 'shared/remote-types';
import { useAuth } from '@/shared/hooks/auth/useAuth';
import { useUserSystem } from '@/shared/hooks/useUserSystem';
import {
  UserContext,
  type UserContextValue,
} from '@/shared/hooks/useUserContext';
import { useCurrentAppDestination } from '@/shared/hooks/useCurrentAppDestination';
import { getElectricShapeSyncOptions } from '@/shared/lib/electric/syncPolicy';

interface UserProviderProps {
  children: ReactNode;
}

export function UserProvider({ children }: UserProviderProps) {
  const { isSignedIn } = useAuth();
  const { liteMode } = useUserSystem();
  const destination = useCurrentAppDestination();

  // No params needed - backend gets user from auth context
  const params = useMemo(() => ({}), []);
  const signedIn = isSignedIn && !liteMode;
  const workspacesEnabled = signedIn;
  const inboxEnabled = signedIn && destination?.kind === 'project-inbox';
  const workspaceSync = getElectricShapeSyncOptions('snapshot');
  const inboxSync = getElectricShapeSyncOptions('live');

  // Shape subscriptions
  const workspacesResult = useShape(USER_WORKSPACES_SHAPE, params, {
    enabled: workspacesEnabled,
    ...workspaceSync,
  });
  const inboxResult = useShape(USER_INBOX_SHAPE, params, {
    enabled: inboxEnabled,
    ...inboxSync,
  });

  // Lookup helpers
  const getWorkspacesForIssue = useCallback(
    (issueId: string) => {
      return workspacesResult.data.filter((w) => w.issue_id === issueId);
    },
    [workspacesResult.data]
  );

  const value = useMemo<UserContextValue>(
    () => ({
      // Data
      workspaces: workspacesResult.data,
      inboxItems: inboxResult.data,

      // Loading/error
      isLoading: workspacesResult.isLoading || inboxResult.isLoading,
      error: workspacesResult.error || inboxResult.error,
      retry: () => {
        workspacesResult.retry();
        inboxResult.retry();
      },

      // Lookup helpers
      getWorkspacesForIssue,
    }),
    [workspacesResult, inboxResult, getWorkspacesForIssue]
  );

  return <UserContext.Provider value={value}>{children}</UserContext.Provider>;
}
