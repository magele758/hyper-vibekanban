import { useEffect } from 'react';
import { useLocation, useNavigate } from '@tanstack/react-router';
import { useUserSystem } from '@/shared/hooks/useUserSystem';
import { isLiteAllowedPath } from '@/shared/lib/liteMode';

export function LiteRouteGuard() {
  const { liteMode } = useUserSystem();
  const location = useLocation();
  const navigate = useNavigate();

  useEffect(() => {
    if (!liteMode || isLiteAllowedPath(location.pathname)) {
      return;
    }
    void navigate({ to: '/workspaces', replace: true });
  }, [liteMode, location.pathname, navigate]);

  return null;
}
