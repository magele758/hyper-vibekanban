import {
  DragDropContext,
  Draggable,
  Droppable,
  type DropResult,
} from '@hello-pangea/dnd';
import type { ReactNode } from 'react';
import {
  LayoutIcon,
  DownloadSimpleIcon,
  LinkIcon,
  PlusIcon,
  KanbanIcon,
  SpinnerIcon,
  RobotIcon,
  ChatCircleIcon,
  TrayIcon,
  type Icon,
} from '@phosphor-icons/react';
import { cn } from '../lib/cn';
import { AppBarSocialLink } from './AppBarSocialLink';
import {
  Popover,
  PopoverTrigger,
  PopoverContent,
  PopoverClose,
} from './Popover';
import { Tooltip } from './Tooltip';
import { useTranslation } from 'react-i18next';

function getProjectInitials(name: string): string {
  const trimmed = name.trim();
  if (!trimmed) return '??';

  const words = trimmed.split(/\s+/);
  if (words.length >= 2) {
    return (words[0].charAt(0) + words[1].charAt(0)).toUpperCase();
  }
  return trimmed.slice(0, 2).toUpperCase();
}

interface AppBarProps {
  projects: AppBarProject[];
  hosts?: AppBarHost[];
  onPairHostClick?: () => void;
  activeHostId?: string | null;
  onCreateProject: () => void;
  onExportClick?: () => void;
  onWorkspacesClick: () => void;
  onHostClick?: (hostId: string, status: AppBarHostStatus) => void;
  showWorkspacesButton?: boolean;
  onProjectClick: (projectId: string) => void;
  onProjectHover?: (projectId: string) => void;
  onProjectsDragEnd: (result: DropResult) => void;
  isSavingProjectOrder?: boolean;
  isWorkspacesActive: boolean;
  isExportActive?: boolean;
  activeProjectId: string | null;
  isSignedIn?: boolean;
  isLoadingProjects?: boolean;
  onSignIn?: () => void;
  onHoverStart?: () => void;
  onHoverEnd?: () => void;
  notificationBell?: ReactNode;
  userPopover?: ReactNode;
  appVersion?: string | null;
  updateVersion?: string | null;
  onUpdateClick?: () => void;
  githubIconPath: string;
  /** Expanded nav tree mode (labels + project sub-nav). */
  expanded?: boolean;
  onToggleExpanded?: () => void;
  onNavigateBoard?: (projectId: string) => void;
  onNavigateAgents?: (projectId: string) => void;
  onNavigateCopilot?: (projectId: string) => void;
  onNavigateInbox?: (projectId: string) => void;
  activeProjectSubNav?: 'board' | 'agents' | 'copilot' | 'inbox' | null;
}

export interface AppBarProject {
  id: string;
  name: string;
  color: string;
}

export type AppBarHostStatus = 'online' | 'offline' | 'unpaired';

export interface AppBarHost {
  id: string;
  name: string;
  status: AppBarHostStatus;
}

function getHostStatusLabel(status: AppBarHostStatus): string {
  if (status === 'online') return 'Online';
  if (status === 'offline') return 'Offline';
  return 'Unpaired';
}

function getHostStatusIndicatorClass(status: AppBarHostStatus): string {
  if (status === 'online') return 'bg-success';
  if (status === 'offline') return 'bg-low';
  return 'bg-white border-warning';
}

function AppBarSectionLabel({
  children,
  expanded,
}: {
  children: ReactNode;
  expanded?: boolean;
}) {
  return (
    <p
      className={cn(
        'font-medium leading-none tracking-wide text-low',
        expanded
          ? 'px-2 text-[10px] text-left'
          : 'w-10 text-center text-[9px]'
      )}
    >
      {children}
    </p>
  );
}

const appBarItemBaseClassName =
  'flex items-center rounded-lg text-sm font-medium transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-brand';

function getAppBarItemLayoutClassName(expanded: boolean) {
  return expanded
    ? 'w-full h-9 justify-start gap-2 px-2.5'
    : 'w-10 h-10 justify-center';
}

function AppBarItemLabel({ children }: { children: ReactNode }) {
  return (
    <span className="min-w-0 flex-1 truncate text-left text-sm font-medium">
      {children}
    </span>
  );
}

type AppBarSection = {
  key: 'local' | 'remote' | 'projects' | 'export';
  label: string;
  items: AppBarSectionItem[];
};

type AppBarSectionItem =
  | {
      key: string;
      kind: 'icon-button';
      label: string;
      icon: Icon;
      isActive?: boolean;
      onClick?: () => void;
      className?: string;
      wrapperClassName?: string;
    }
  | {
      key: string;
      kind: 'host-button';
      host: AppBarHost;
      isActive: boolean;
      onClick?: () => void;
      wrapperClassName?: string;
    }
  | {
      key: string;
      kind: 'kanban-cta';
      label: string;
      onSignIn?: () => void;
    }
  | {
      key: string;
      kind: 'loading';
    }
  | {
      key: string;
      kind: 'project-list';
      projects: AppBarProject[];
      activeProjectId: string | null;
      isSavingProjectOrder?: boolean;
      onProjectClick: (projectId: string) => void;
      onProjectHover?: (projectId: string) => void;
      onProjectsDragEnd: (result: DropResult) => void;
    };

function getStandardAppBarButtonClassName({
  isActive = false,
  className,
  expanded = false,
}: {
  isActive?: boolean;
  className?: string;
  expanded?: boolean;
}) {
  return cn(
    appBarItemBaseClassName,
    getAppBarItemLayoutClassName(expanded),
    'cursor-pointer',
    isActive
      ? 'bg-brand/20 text-brand hover:bg-brand/20'
      : 'bg-primary text-normal hover:bg-brand/10',
    className
  );
}

function getHostButtonClassName({
  host,
  isActive,
  expanded = false,
}: {
  host: AppBarHost;
  isActive: boolean;
  expanded?: boolean;
}) {
  const isOffline = host.status === 'offline';

  return cn(
    appBarItemBaseClassName,
    getAppBarItemLayoutClassName(expanded),
    isOffline
      ? 'bg-primary text-low opacity-50 cursor-not-allowed'
      : isActive
        ? 'bg-brand/20 text-brand cursor-pointer hover:bg-brand/20'
        : host.status === 'unpaired'
          ? 'bg-primary text-warning cursor-pointer hover:bg-warning/10'
          : 'bg-primary text-normal cursor-pointer hover:bg-brand/10'
  );
}

export function AppBar({
  projects,
  hosts = [],
  onPairHostClick,
  activeHostId = null,
  onCreateProject,
  onExportClick,
  onWorkspacesClick,
  onHostClick,
  showWorkspacesButton = true,
  onProjectClick,
  onProjectHover,
  onProjectsDragEnd,
  isSavingProjectOrder,
  isWorkspacesActive,
  isExportActive = false,
  activeProjectId,
  isSignedIn,
  isLoadingProjects,
  onSignIn,
  onHoverStart,
  onHoverEnd,
  notificationBell,
  userPopover,
  appVersion,
  updateVersion,
  onUpdateClick,
  githubIconPath,
  expanded = false,
  onToggleExpanded,
  onNavigateBoard,
  onNavigateAgents,
  onNavigateCopilot,
  onNavigateInbox,
  activeProjectSubNav = null,
}: AppBarProps) {
  const { t } = useTranslation('common');
  const sections: AppBarSection[] = [];

  if (showWorkspacesButton) {
    sections.push({
      key: 'local',
      label: 'Local',
      items: [
        {
          key: 'local-workspaces',
          kind: 'icon-button',
          label: 'Local workspaces',
          icon: LayoutIcon,
          isActive: isWorkspacesActive,
          onClick: onWorkspacesClick,
        },
      ],
    });
  }

  if (hosts.length > 0 || onPairHostClick) {
    sections.push({
      key: 'remote',
      label: 'Remote',
      items: [
        ...hosts.map((host) => ({
          key: `host-${host.id}`,
          kind: 'host-button' as const,
          host,
          isActive: host.id === activeHostId,
          onClick: () => {
            if (host.status === 'offline') {
              return;
            }

            onHostClick?.(host.id, host.status);
          },
        })),
        ...(onPairHostClick
          ? [
              {
                key: 'pair-remote-device',
                kind: 'icon-button' as const,
                label: 'Pair a remote device',
                icon: LinkIcon,
                onClick: onPairHostClick,
                className:
                  'bg-primary text-muted hover:text-normal hover:bg-tertiary',
              },
            ]
          : []),
      ],
    });
  }

  const projectSectionItems: AppBarSectionItem[] = [];

  if (!isSignedIn) {
    projectSectionItems.push({
      key: 'kanban-cta',
      kind: 'kanban-cta',
      label: t('appBar.kanban.tooltip'),
      onSignIn,
    });
  }

  if (isLoadingProjects) {
    projectSectionItems.push({ key: 'projects-loading', kind: 'loading' });
  }

  if (projects.length > 0) {
    projectSectionItems.push({
      key: 'project-list',
      kind: 'project-list',
      projects,
      activeProjectId,
      isSavingProjectOrder,
      onProjectClick,
      onProjectHover,
      onProjectsDragEnd,
    });
  }

  if (isSignedIn) {
    projectSectionItems.push({
      key: 'create-project',
      kind: 'icon-button',
      label: 'Create project',
      icon: PlusIcon,
      onClick: onCreateProject,
      className: 'bg-primary text-muted hover:text-normal hover:bg-tertiary',
      wrapperClassName: 'pt-base',
    });
  }

  if (projectSectionItems.length > 0) {
    sections.push({
      key: 'projects',
      label: 'Projects',
      items: projectSectionItems,
    });
  }

  if (isSignedIn && onExportClick) {
    sections.push({
      key: 'export',
      label: 'Export',
      items: [
        {
          key: 'export-data',
          kind: 'icon-button',
          label: 'Export data',
          icon: DownloadSimpleIcon,
          isActive: isExportActive,
          onClick: onExportClick,
        },
      ],
    });
  }

  function renderSectionItem(item: AppBarSectionItem): ReactNode {
    switch (item.kind) {
      case 'icon-button': {
        const button = (
          <button
            type="button"
            onClick={item.onClick}
            className={getStandardAppBarButtonClassName({
              isActive: item.isActive,
              className: item.className,
              expanded,
            })}
            aria-label={item.label}
            title={expanded ? item.label : undefined}
          >
            <item.icon
              className="size-icon-base shrink-0"
              weight="bold"
            />
            {expanded && <AppBarItemLabel>{item.label}</AppBarItemLabel>}
          </button>
        );
        return expanded ? (
          button
        ) : (
          <Tooltip content={item.label} side="right">
            {button}
          </Tooltip>
        );
      }
      case 'host-button': {
        const isOffline = item.host.status === 'offline';
        const button = (
          <div className={cn('relative', expanded && 'w-full')}>
            <span
              className={cn(
                'absolute z-10 w-3.5 h-3.5 rounded-full border border-secondary',
                expanded ? 'top-1 right-1' : '-top-1 -right-1',
                getHostStatusIndicatorClass(item.host.status)
              )}
              aria-hidden="true"
            />
            <button
              type="button"
              disabled={isOffline}
              onClick={item.onClick}
              className={getHostButtonClassName({
                host: item.host,
                isActive: item.isActive,
                expanded,
              })}
              aria-label={`${item.host.name} (${getHostStatusLabel(item.host.status)})`}
              title={
                expanded
                  ? `${item.host.name} · ${getHostStatusLabel(item.host.status)}`
                  : undefined
              }
            >
              <span
                className={cn(
                  'flex shrink-0 items-center justify-center font-medium',
                  expanded ? 'size-6 rounded-md bg-secondary text-xs' : 'text-sm'
                )}
              >
                {getProjectInitials(item.host.name)}
              </span>
              {expanded && <AppBarItemLabel>{item.host.name}</AppBarItemLabel>}
            </button>
          </div>
        );
        return expanded ? (
          button
        ) : (
          <Tooltip
            content={`${item.host.name} · ${getHostStatusLabel(item.host.status)}`}
            side="right"
          >
            {button}
          </Tooltip>
        );
      }
      case 'kanban-cta': {
        const trigger = (
          <PopoverTrigger asChild>
            <button
              type="button"
              className={getStandardAppBarButtonClassName({ expanded })}
              aria-label={item.label}
              title={expanded ? item.label : undefined}
            >
              <KanbanIcon
                className="size-icon-base shrink-0"
                weight="bold"
              />
              {expanded && <AppBarItemLabel>{item.label}</AppBarItemLabel>}
            </button>
          </PopoverTrigger>
        );
        return (
          <Popover>
            {expanded ? (
              trigger
            ) : (
              <Tooltip content={item.label} side="right">
                {trigger}
              </Tooltip>
            )}
            <PopoverContent side="right" sideOffset={8}>
              <p className="text-sm font-medium text-high">
                {t('appBar.kanban.title')}
              </p>
              <p className="text-xs text-low mt-1">
                {t('appBar.kanban.description')}
              </p>
              <div className="mt-base">
                <PopoverClose asChild>
                  <button
                    type="button"
                    onClick={item.onSignIn}
                    className={cn(
                      'px-base py-1 rounded-sm text-xs',
                      'bg-brand text-on-brand hover:bg-brand-hover cursor-pointer'
                    )}
                  >
                    {t('signIn')}
                  </button>
                </PopoverClose>
              </div>
            </PopoverContent>
          </Popover>
        );
      }
      case 'loading':
        return (
          <div
            className={cn(
              'flex items-center',
              expanded ? 'h-9 w-full justify-start gap-2 px-2.5' : 'h-10 w-10 justify-center'
            )}
          >
            <SpinnerIcon className="size-5 shrink-0 animate-spin text-muted" />
            {expanded && (
              <AppBarItemLabel>Loading…</AppBarItemLabel>
            )}
          </div>
        );
      case 'project-list':
        return (
          <DragDropContext onDragEnd={item.onProjectsDragEnd}>
            <Droppable
              droppableId="app-bar-projects"
              direction="vertical"
              isDropDisabled={item.isSavingProjectOrder}
            >
              {(dropProvided) => (
                <div
                  ref={dropProvided.innerRef}
                  {...dropProvided.droppableProps}
                  className={cn(
                    'flex flex-col -mb-base',
                    expanded ? 'items-stretch' : 'items-center'
                  )}
                >
                  {item.projects.map((project, index) => (
                    <Draggable
                      key={project.id}
                      draggableId={project.id}
                      index={index}
                      disableInteractiveElementBlocking
                      isDragDisabled={item.isSavingProjectOrder}
                    >
                      {(dragProvided, snapshot) => {
                        const projectButton = (
                          <button
                            type="button"
                            onClick={() => item.onProjectClick(project.id)}
                            onMouseEnter={() =>
                              item.onProjectHover?.(project.id)
                            }
                            onFocus={() => item.onProjectHover?.(project.id)}
                            className={cn(
                              appBarItemBaseClassName,
                              getAppBarItemLayoutClassName(expanded),
                              'cursor-grab',
                              snapshot.isDragging && 'shadow-lg',
                              item.activeProjectId === project.id
                                ? ''
                                : 'bg-primary text-normal hover:opacity-80'
                            )}
                            style={
                              item.activeProjectId === project.id
                                ? {
                                    color: `hsl(${project.color})`,
                                    backgroundColor: `hsl(${project.color} / 0.2)`,
                                  }
                                : undefined
                            }
                            aria-label={project.name}
                            title={expanded ? project.name : undefined}
                          >
                            <span
                              className={cn(
                                'flex shrink-0 items-center justify-center font-medium',
                                expanded
                                  ? 'size-6 rounded-md text-xs'
                                  : 'text-sm'
                              )}
                              style={
                                expanded
                                  ? {
                                      color: `hsl(${project.color})`,
                                      backgroundColor: `hsl(${project.color} / 0.2)`,
                                    }
                                  : undefined
                              }
                            >
                              {getProjectInitials(project.name)}
                            </span>
                            {expanded && (
                              <AppBarItemLabel>{project.name}</AppBarItemLabel>
                            )}
                          </button>
                        );

                        return (
                          <div
                            ref={dragProvided.innerRef}
                            {...dragProvided.draggableProps}
                            {...dragProvided.dragHandleProps}
                            className="mb-base"
                            style={dragProvided.draggableProps.style}
                          >
                            {expanded ? (
                              projectButton
                            ) : (
                              <Tooltip content={project.name} side="right">
                                {projectButton}
                              </Tooltip>
                            )}
                          </div>
                        );
                      }}
                    </Draggable>
                  ))}
                  {dropProvided.placeholder}
                </div>
              )}
            </Droppable>
          </DragDropContext>
        );
    }
  }

  return (
    <div
      onMouseEnter={onHoverStart}
      onMouseLeave={onHoverEnd}
      className={cn(
        'flex flex-col h-full min-h-0 overflow-y-auto p-base gap-base',
        'bg-secondary border-r border-border transition-[width]',
        expanded ? 'w-[220px] items-stretch' : 'w-auto items-center'
      )}
    >
      {onToggleExpanded && (
        <button
          type="button"
          onClick={onToggleExpanded}
          className={cn(
            'rounded-md px-2 py-1.5 text-xs text-low hover:text-normal hover:bg-primary',
            expanded ? 'text-left' : 'w-10 text-center'
          )}
          aria-label={expanded ? 'Collapse sidebar' : 'Expand sidebar'}
        >
          {expanded ? '« 收起' : '»'}
        </button>
      )}

      {sections.map((section) => (
        <div
          key={section.key}
          className={cn(
            'flex flex-col gap-1',
            expanded ? 'items-stretch' : 'items-center'
          )}
        >
          <AppBarSectionLabel expanded={expanded}>
            {section.label}
          </AppBarSectionLabel>
          {section.items.map((item) => (
            <div
              key={item.key}
              className={
                'wrapperClassName' in item ? item.wrapperClassName : undefined
              }
            >
              {renderSectionItem(item)}
            </div>
          ))}
        </div>
      ))}

      {expanded && activeProjectId && (
        <div className="flex flex-col gap-1 border-t border-border pt-base">
          <AppBarSectionLabel expanded>Project</AppBarSectionLabel>
          {(
            [
              {
                id: 'board' as const,
                label: 'Board',
                icon: KanbanIcon,
                onClick: () => onNavigateBoard?.(activeProjectId),
              },
              {
                id: 'agents' as const,
                label: 'Agents',
                icon: RobotIcon,
                onClick: () => onNavigateAgents?.(activeProjectId),
              },
              {
                id: 'copilot' as const,
                label: 'Copilot',
                icon: ChatCircleIcon,
                onClick: () => onNavigateCopilot?.(activeProjectId),
              },
              {
                id: 'inbox' as const,
                label: 'Inbox',
                icon: TrayIcon,
                onClick: () => onNavigateInbox?.(activeProjectId),
              },
            ] as const
          ).map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={item.onClick}
              title={item.label}
              className={cn(
                appBarItemBaseClassName,
                getAppBarItemLayoutClassName(true),
                activeProjectSubNav === item.id
                  ? 'bg-brand/15 text-normal'
                  : 'text-low hover:bg-primary hover:text-normal'
              )}
            >
              <item.icon className="size-icon-base shrink-0" weight="bold" />
              <AppBarItemLabel>{item.label}</AppBarItemLabel>
            </button>
          ))}
        </div>
      )}

      {/* Bottom section: Notifications + User popover + GitHub */}
      <div
        className={cn(
          'mt-auto pt-base flex flex-col gap-4',
          expanded ? 'items-stretch' : 'items-center'
        )}
      >
        {notificationBell}
        {userPopover}
        <AppBarSocialLink
          href="https://github.com/magele758/hyper-vibekanban"
          label="Star on GitHub"
          iconPath={githubIconPath}
          expanded={expanded}
        />
        {updateVersion ? (
          expanded ? (
            <button
              type="button"
              onClick={onUpdateClick}
              title={`Update to v${updateVersion}`}
              className={cn(
                'flex h-9 w-full items-center justify-start gap-2 rounded-md px-2.5',
                'text-xs font-ibm-plex-mono font-medium',
                'bg-brand text-on-brand hover:bg-brand-hover',
                'transition-colors cursor-pointer'
              )}
            >
              <span className="min-w-0 flex-1 truncate text-left">
                Update to v{updateVersion}
              </span>
            </button>
          ) : (
            <Tooltip content={`Update to v${updateVersion}`} side="right">
              <button
                type="button"
                onClick={onUpdateClick}
                className={cn(
                  'flex items-center justify-center py-1 rounded-md w-10',
                  'text-[9px] font-ibm-plex-mono font-medium leading-none',
                  'bg-brand text-on-brand hover:bg-brand-hover',
                  'transition-colors cursor-pointer'
                )}
              >
                Update
              </button>
            </Tooltip>
          )
        ) : (
          appVersion && (
            <p
              className={cn(
                'font-ibm-plex-mono text-low leading-none truncate',
                expanded
                  ? 'px-2.5 text-[10px] text-left'
                  : 'max-w-10 text-center text-[9px]'
              )}
              title={`v${appVersion}`}
            >
              v{appVersion}
            </p>
          )
        )}
      </div>
    </div>
  );
}
