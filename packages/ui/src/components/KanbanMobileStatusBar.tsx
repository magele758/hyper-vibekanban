'use client';

import { cn } from '../lib/cn';

export type KanbanMobileStatus = {
  id: string;
  name: string;
  color: string;
  count: number;
};

export interface KanbanMobileStatusBarProps {
  statuses: KanbanMobileStatus[];
  selectedStatusId: string;
  onSelect: (statusId: string) => void;
  className?: string;
}

/**
 * Horizontal status pills for mobile single-column kanban.
 * One status is shown at a time; tapping a pill switches the active column.
 */
export function KanbanMobileStatusBar({
  statuses,
  selectedStatusId,
  onSelect,
  className,
}: KanbanMobileStatusBarProps) {
  return (
    <div
      className={cn(
        'flex gap-half overflow-x-auto pb-half -mx-base px-base scrollbar-none',
        className
      )}
      role="tablist"
      aria-label="Status columns"
    >
      {statuses.map((status) => {
        const isSelected = status.id === selectedStatusId;
        return (
          <button
            key={status.id}
            type="button"
            role="tab"
            aria-selected={isSelected}
            onClick={() => onSelect(status.id)}
            className={cn(
              'shrink-0 inline-flex items-center gap-half rounded-sm border px-base py-half text-sm transition-colors',
              isSelected
                ? 'border-accent bg-accent/10 text-normal'
                : 'border-border bg-primary text-low hover:text-normal hover:bg-secondary'
            )}
          >
            <span
              className="h-2 w-2 rounded-full shrink-0"
              style={{ backgroundColor: `hsl(${status.color})` }}
            />
            <span className="whitespace-nowrap">{status.name}</span>
            <span
              className={cn(
                'font-ibm-plex-mono text-xs tabular-nums',
                isSelected ? 'text-normal' : 'text-low'
              )}
            >
              {status.count}
            </span>
          </button>
        );
      })}
    </div>
  );
}
