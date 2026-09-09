import { type ReactNode } from 'react';
import { RouterProvider } from '@tanstack/react-router';
import { HotkeysProvider } from 'react-hotkeys-hook';
import { UserSystemProvider } from '@web/app/providers/ConfigProvider';
import { ClickedElementsProvider } from '@web/app/providers/ClickedElementsProvider';
import { localAppNavigation } from '@web/app/navigation/AppNavigation';
import { LocalAuthProvider } from '@/shared/providers/auth/LocalAuthProvider';
import { AppRuntimeProvider } from '@/shared/hooks/useAppRuntime';
import { AppNavigationProvider } from '@/shared/hooks/useAppNavigation';
import { useUserSystem } from '@/shared/hooks/useUserSystem';
import { useTauriNotificationNavigation } from '@web/app/hooks/useTauriNotificationNavigation';
import { useTauriUpdateReady } from '@web/app/hooks/useTauriUpdateReady';
import { AppSystemNotifications } from '@web/app/notifications/AppSystemNotifications';
import { router } from '@web/app/router';

function TauriListeners() {
  useTauriNotificationNavigation();
  useTauriUpdateReady();
  return null;
}

function AppHotkeys({ children }: { children: ReactNode }) {
  const { liteMode } = useUserSystem();
  return (
    <HotkeysProvider
      initiallyActiveScopes={
        liteMode
          ? ['global', 'workspace']
          : ['global', 'workspace', 'kanban', 'projects']
      }
    >
      {children}
    </HotkeysProvider>
  );
}

function App() {
  return (
    <AppRuntimeProvider runtime="local">
      <AppNavigationProvider value={localAppNavigation}>
        <TauriListeners />
        <UserSystemProvider>
          <LocalAuthProvider>
            <AppSystemNotifications />
            <ClickedElementsProvider>
              <AppHotkeys>
                <RouterProvider router={router} />
              </AppHotkeys>
            </ClickedElementsProvider>
          </LocalAuthProvider>
        </UserSystemProvider>
      </AppNavigationProvider>
    </AppRuntimeProvider>
  );
}

export default App;
