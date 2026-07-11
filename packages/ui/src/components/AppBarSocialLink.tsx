import type { ReactNode } from 'react';
import { cn } from '../lib/cn';
import { Tooltip } from './Tooltip';

interface AppBarSocialLinkProps {
  href: string;
  label: string;
  iconPath: string;
  badge?: ReactNode;
  /** When true, show icon + truncated label instead of icon-only. */
  expanded?: boolean;
}

export function AppBarSocialLink({
  href,
  label,
  iconPath,
  badge,
  expanded = false,
}: AppBarSocialLinkProps) {
  const link = (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      className={cn(
        'relative flex items-center rounded-lg',
        'text-sm font-medium transition-colors cursor-pointer',
        'focus:outline-none focus-visible:ring-2 focus-visible:ring-brand',
        'bg-panel text-normal hover:opacity-80',
        expanded
          ? 'h-9 w-full justify-start gap-2 px-2.5'
          : 'h-10 w-10 justify-center'
      )}
      aria-label={label}
      title={expanded ? label : undefined}
    >
      <svg
        className="h-5 w-5 shrink-0"
        viewBox="0 0 24 24"
        fill="currentColor"
        aria-hidden="true"
      >
        <path d={iconPath} />
      </svg>
      {expanded && (
        <span className="min-w-0 flex-1 truncate text-left text-sm font-medium">
          {label}
        </span>
      )}
      {badge != null && badge !== false && (
        <span
          className={cn(
            'min-w-[18px] h-[18px] px-1 flex items-center justify-center gap-0.5 rounded-full',
            'bg-brand-secondary text-[10px] font-medium text-white',
            expanded ? 'ml-auto' : 'absolute -top-2 -right-1'
          )}
        >
          {badge}
        </span>
      )}
    </a>
  );

  return expanded ? (
    link
  ) : (
    <Tooltip content={label} side="right">
      {link}
    </Tooltip>
  );
}
