import { useNavigate } from '@tanstack/react-router';
import { BellIcon } from '@phosphor-icons/react';
import { cn } from '@vibe/ui/lib/cn';
import { Tooltip } from '@vibe/ui/components/Tooltip';
import { useNotifications } from '@/shared/hooks/useNotifications';

interface AppBarNotificationBellContainerProps {
  /** Match AppBar expand/collapse layout (injected by AppBar). */
  expanded?: boolean;
}

export function AppBarNotificationBellContainer({
  expanded = false,
}: AppBarNotificationBellContainerProps) {
  const navigate = useNavigate();
  const { unseenCount, enabled } = useNotifications();

  if (!enabled) return null;

  const button = (
    <button
      type="button"
      onClick={() => navigate({ to: '/notifications' })}
      className={cn(
        'relative flex items-center rounded-lg',
        'text-sm font-medium transition-colors cursor-pointer',
        'focus:outline-none focus-visible:ring-2 focus-visible:ring-brand',
        'bg-panel text-normal hover:opacity-80',
        expanded
          ? 'h-9 w-full justify-start gap-2 px-2.5'
          : 'h-10 w-10 justify-center'
      )}
      aria-label="Notifications"
      title={expanded ? 'Notifications' : undefined}
    >
      <BellIcon className="h-5 w-5 shrink-0" weight="bold" />
      {expanded && (
        <span className="min-w-0 flex-1 truncate text-left text-sm font-medium">
          Notifications
        </span>
      )}
      {unseenCount > 0 && (
        <span
          className={cn(
            'min-w-[18px] h-[18px] px-1 flex items-center justify-center rounded-full',
            'bg-brand-secondary text-[10px] font-medium text-white',
            expanded ? 'ml-auto' : 'absolute -top-2 -right-1'
          )}
        >
          {unseenCount > 99 ? '99+' : unseenCount}
        </span>
      )}
    </button>
  );

  return expanded ? (
    button
  ) : (
    <Tooltip content="Notifications" side="right">
      {button}
    </Tooltip>
  );
}
