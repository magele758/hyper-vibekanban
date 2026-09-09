import { useEffect } from 'react';
import { useLocation, useNavigate } from '@tanstack/react-router';
import { useUserSystem } from '@/shared/hooks/useUserSystem';
import {
  isLiteAllowedPath,
  liteCanonicalProjectPath,
} from '@/shared/lib/liteMode';

export function LiteRouteGuard() {
  const { liteMode } = useUserSystem();
  const location = useLocation();
  const navigate = useNavigate();

  useEffect(() => {
    if (!liteMode) {
      return;
    }

    const canonical = liteCanonicalProjectPath(location.pathname);
    if (canonical) {
      const projectId = canonical.slice('/projects/'.length);
      void navigate({
        to: '/projects/$projectId',
        params: { projectId },
        replace: true,
      });
      return;
    }

    if (isLiteAllowedPath(location.pathname)) {
      return;
    }
    void navigate({ to: '/workspaces', replace: true });
  }, [liteMode, location.pathname, navigate]);

  return null;
}
