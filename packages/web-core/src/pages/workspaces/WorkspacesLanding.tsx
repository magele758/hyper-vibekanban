import { useEffect, useLayoutEffect } from 'react';
import { SpinnerIcon } from '@phosphor-icons/react';
import { MobileWorkspacesList } from '@/pages/workspaces/MobileWorkspacesList';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { useAppRuntime } from '@/shared/hooks/useAppRuntime';
import { useCurrentAppDestination } from '@/shared/hooks/useCurrentAppDestination';
import { useExecutionHostId } from '@/shared/hooks/useExecutionHostId';
import { useIsMobile } from '@/shared/hooks/useIsMobile';
import { getDestinationHostId } from '@/shared/lib/routes/appNavigation';

export function WorkspacesLanding() {
  const appNavigation = useAppNavigation();
  const runtime = useAppRuntime();
  const destination = useCurrentAppDestination();
  const isMobile = useIsMobile();
  const { setExecutionHostId } = useExecutionHostId();
  const destinationHostId = getDestinationHostId(destination);

  // Sync from the resolved destination (not useParams) before redirecting to
  // create — useParams({strict:false}) can retain a stale hostId after leaving
  // /hosts/{id}/... and would bounce the selection back to the remote host.
  useLayoutEffect(() => {
    if (runtime !== 'local') {
      return;
    }
    setExecutionHostId(destinationHostId);
  }, [runtime, destinationHostId, setExecutionHostId]);

  // Desktop: /workspaces is a redirect into create. Mobile needs the list so
  // users can open remote-host workspaces from the drawer (desktop AppBar
  // already exposes hosts; mobile previously had no list surface).
  useEffect(() => {
    if (isMobile) {
      return;
    }
    appNavigation.goToWorkspacesCreate({
      replace: true,
    });
  }, [appNavigation, isMobile]);

  if (isMobile) {
    return <MobileWorkspacesList />;
  }

  return (
    <div className="flex h-full flex-1 items-center justify-center bg-primary">
      <SpinnerIcon className="size-6 animate-spin text-low" />
    </div>
  );
}
