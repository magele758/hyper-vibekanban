import {
  BuildingsIcon,
  CheckIcon,
  GearIcon,
  SignInIcon,
  SignOutIcon,
  UserIcon,
} from '@phosphor-icons/react';
import { useTranslation } from 'react-i18next';
import { cn } from '../lib/cn';
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
} from './Dropdown';

export interface AppBarUserOrganization {
  id: string;
  name: string;
}

interface AppBarUserPopoverProps {
  isSignedIn: boolean;
  avatarUrl: string | null;
  avatarError: boolean;
  organizations: AppBarUserOrganization[];
  selectedOrgId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onOrgSelect: (orgId: string) => void;
  onOrgSettings?: (orgId: string) => void;
  onSettings?: () => void;
  onSignIn: () => void;
  onLogout: () => void;
  onAvatarError: () => void;
  /** Match AppBar expand/collapse layout (injected by AppBar). */
  expanded?: boolean;
}

export function AppBarUserPopover({
  isSignedIn,
  avatarUrl,
  avatarError,
  organizations,
  selectedOrgId,
  open,
  onOpenChange,
  onOrgSelect,
  onOrgSettings,
  onSettings,
  onSignIn,
  onLogout,
  onAvatarError,
  expanded = false,
}: AppBarUserPopoverProps) {
  const { t } = useTranslation();
  const settingsLabel = t('settings:settings.layout.nav.title', {
    defaultValue: 'Settings',
  });
  const selectedOrgName =
    organizations.find((org) => org.id === selectedOrgId)?.name ?? null;

  const triggerClassName = cn(
    'flex items-center rounded-md sm:rounded-lg',
    'transition-colors cursor-pointer',
    'focus:outline-none focus-visible:ring-2 focus-visible:ring-brand',
    expanded
      ? 'h-9 w-full justify-start gap-2 px-2.5'
      : 'w-7 h-7 sm:w-10 sm:h-10 justify-center overflow-hidden',
    (!avatarUrl || avatarError || !isSignedIn) &&
      'bg-panel text-normal font-medium text-sm',
    (!avatarUrl || avatarError || !isSignedIn) && 'hover:bg-panel/70'
  );

  if (!isSignedIn) {
    return (
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button
            type="button"
            className={triggerClassName}
            aria-label="Sign in"
            title={expanded ? 'Sign in' : undefined}
          >
            <UserIcon className="size-icon-sm shrink-0" weight="bold" />
            {expanded && (
              <span className="min-w-0 flex-1 truncate text-left text-sm font-medium">
                Sign in
              </span>
            )}
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent side="right" align="end" className="min-w-[200px]">
          <DropdownMenuItem icon={SignInIcon} onClick={onSignIn}>
            {t('signIn')}
          </DropdownMenuItem>
          {onSettings && (
            <>
              <DropdownMenuSeparator />
              <DropdownMenuItem icon={GearIcon} onClick={onSettings}>
                {settingsLabel}
              </DropdownMenuItem>
            </>
          )}
        </DropdownMenuContent>
      </DropdownMenu>
    );
  }

  return (
    <DropdownMenu open={open} onOpenChange={onOpenChange}>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          className={triggerClassName}
          aria-label="Account"
          title={expanded ? (selectedOrgName ?? 'Account') : undefined}
        >
          {avatarUrl && !avatarError ? (
            <img
              src={avatarUrl}
              alt="User avatar"
              className={cn(
                'object-cover shrink-0',
                expanded ? 'size-5 rounded' : 'w-full h-full'
              )}
              onError={onAvatarError}
            />
          ) : (
            <UserIcon className="size-icon-sm shrink-0" weight="bold" />
          )}
          {expanded && (
            <span className="min-w-0 flex-1 truncate text-left text-sm font-medium">
              {selectedOrgName ?? 'Account'}
            </span>
          )}
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent side="right" align="end" className="min-w-[200px]">
        <DropdownMenuLabel>{t('orgSwitcher.organizations')}</DropdownMenuLabel>
        <DropdownMenuSeparator />
        {organizations.map((org) => (
          <DropdownMenuItem
            key={org.id}
            icon={org.id === selectedOrgId ? CheckIcon : BuildingsIcon}
            onClick={() => onOrgSelect(org.id)}
            className={cn(org.id === selectedOrgId && 'bg-brand/10', 'group')}
          >
            <span className="flex items-center gap-2 w-full">
              <span className="flex-1 truncate">{org.name}</span>
              {onOrgSettings && (
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onOpenChange(false);
                    onOrgSettings(org.id);
                  }}
                  className="sm:opacity-0 sm:group-hover:opacity-100 p-1 rounded hover:bg-secondary transition-opacity shrink-0"
                  aria-label={t('orgSwitcher.orgSettings')}
                >
                  <GearIcon className="size-icon-xs" weight="bold" />
                </button>
              )}
            </span>
          </DropdownMenuItem>
        ))}
        {onSettings && (
          <>
            <DropdownMenuSeparator />
            <DropdownMenuItem icon={GearIcon} onClick={onSettings}>
              {settingsLabel}
            </DropdownMenuItem>
          </>
        )}
        <DropdownMenuSeparator />
        <DropdownMenuItem icon={SignOutIcon} onClick={onLogout}>
          {t('signOut')}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
