import { useState } from 'react';
import {
  PlusIcon,
  GitBranchIcon,
  HandIcon,
  TriangleIcon,
  PlayIcon,
  FileIcon,
  CircleIcon,
  GitPullRequestIcon,
  PushPinIcon,
  DotsThreeIcon,
  ArchiveIcon,
  ArrowLeftIcon,
} from '@phosphor-icons/react';
import { RunningDots } from '@vibe/ui/components/RunningDots';
import { CommandBarDialog } from '@/shared/dialogs/command-bar/CommandBarDialog';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { useWorkspaceContext } from '@/shared/hooks/useWorkspaceContext';
import { cn } from '@/shared/lib/utils';

const formatRelativeElapsed = (dateString: string): string => {
  const date = new Date(dateString);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSecs = Math.floor(diffMs / 1000);
  const diffMins = Math.floor(diffSecs / 60);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffSecs < 60) return 'just now';
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  return `${diffDays}d ago`;
};

export function MobileWorkspacesList() {
  const appNavigation = useAppNavigation();
  const { activeWorkspaces, archivedWorkspaces, selectWorkspace } =
    useWorkspaceContext();
  const [showArchive, setShowArchive] = useState(false);
  const workspaces = showArchive ? archivedWorkspaces : activeWorkspaces;

  const handleSelectWorkspace = (id: string) => {
    selectWorkspace(id);
    appNavigation.goToWorkspace(id);
  };

  const handleCreateWorkspace = () => {
    appNavigation.goToWorkspacesCreate();
  };

  return (
    <div className="flex flex-col h-full bg-primary">
      <div className="flex items-center justify-between px-base py-base border-b border-border">
        <h1 className="text-lg font-semibold text-high">
          {showArchive ? 'Archived' : 'Workspaces'}
        </h1>
        <button
          type="button"
          onClick={handleCreateWorkspace}
          className={cn(
            'flex items-center gap-half rounded-md px-plusfifty py-half',
            'bg-brand text-on-brand text-sm font-medium',
            'active:opacity-80 transition-opacity'
          )}
        >
          <PlusIcon className="size-icon-sm" />
          New
        </button>
      </div>

      <div className="flex-1 overflow-y-auto">
        {workspaces.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full px-double text-center">
            <p className="text-low text-sm">
              {showArchive ? 'No archived workspaces' : 'No workspaces yet'}
            </p>
            {!showArchive && (
              <button
                type="button"
                onClick={handleCreateWorkspace}
                className="mt-base text-brand text-sm font-medium active:opacity-80"
              >
                Create your first workspace
              </button>
            )}
          </div>
        ) : (
          <div className="flex flex-col">
            {workspaces.map((workspace) => {
              const isFailed =
                workspace.latestProcessStatus === 'failed' ||
                workspace.latestProcessStatus === 'killed';
              const hasChanges =
                workspace.filesChanged !== undefined &&
                workspace.filesChanged > 0;

              return (
                <div
                  key={workspace.id}
                  className={cn(
                    'group relative flex items-center gap-half px-base py-plusfifty',
                    'border-b border-border'
                  )}
                >
                  <button
                    type="button"
                    onClick={() => handleSelectWorkspace(workspace.id)}
                    className={cn(
                      'flex flex-1 flex-col gap-half min-w-0',
                      'text-left active:bg-secondary transition-colors'
                    )}
                  >
                    <span className="text-sm font-medium text-high truncate">
                      {workspace.name}
                    </span>
                    <span className="flex items-center gap-base text-xs text-low">
                      {workspace.branch && (
                        <span className="flex items-center gap-half min-w-0 shrink truncate">
                          <GitBranchIcon className="size-icon-xs shrink-0" />
                          <span className="truncate">{workspace.branch}</span>
                        </span>
                      )}

                      <span className="flex items-center gap-half shrink-0">
                        {workspace.hasRunningDevServer && (
                          <PlayIcon
                            className="size-icon-xs text-brand shrink-0"
                            weight="fill"
                          />
                        )}

                        {!workspace.isRunning && isFailed && (
                          <TriangleIcon
                            className="size-icon-xs text-error shrink-0"
                            weight="fill"
                          />
                        )}

                        {workspace.isRunning &&
                          (workspace.hasPendingApproval ? (
                            <HandIcon
                              className="size-icon-xs text-brand shrink-0"
                              weight="fill"
                            />
                          ) : (
                            <RunningDots />
                          ))}

                        {workspace.hasUnseenActivity &&
                          !workspace.isRunning &&
                          !isFailed && (
                            <CircleIcon
                              className="size-icon-xs text-brand shrink-0"
                              weight="fill"
                            />
                          )}

                        {workspace.prStatus === 'open' && (
                          <GitPullRequestIcon
                            className="size-icon-xs text-success shrink-0"
                            weight="fill"
                          />
                        )}
                        {workspace.prStatus === 'merged' && (
                          <GitPullRequestIcon
                            className="size-icon-xs text-merged shrink-0"
                            weight="fill"
                          />
                        )}

                        {workspace.isPinned && (
                          <PushPinIcon
                            className="size-icon-xs text-brand shrink-0"
                            weight="fill"
                          />
                        )}
                      </span>

                      {!workspace.isRunning &&
                        workspace.latestProcessCompletedAt && (
                          <span className="shrink-0">
                            {formatRelativeElapsed(
                              workspace.latestProcessCompletedAt
                            )}
                          </span>
                        )}

                      {hasChanges && (
                        <span className="shrink-0 flex items-center gap-half">
                          <FileIcon className="size-icon-xs" weight="fill" />
                          <span>{workspace.filesChanged}</span>
                          {workspace.linesAdded !== undefined && (
                            <span className="text-success">
                              +{workspace.linesAdded}
                            </span>
                          )}
                          {workspace.linesRemoved !== undefined && (
                            <span className="text-error">
                              -{workspace.linesRemoved}
                            </span>
                          )}
                        </span>
                      )}
                    </span>
                  </button>
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      void CommandBarDialog.show({
                        page: 'workspaceActions',
                        workspaceId: workspace.id,
                      });
                    }}
                    className="shrink-0 p-1.5 rounded-sm text-low hover:text-normal hover:bg-tertiary active:bg-tertiary"
                    aria-label="Workspace actions"
                  >
                    <DotsThreeIcon className="size-5" weight="bold" />
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>

      <div className="border-t border-border p-base">
        <button
          type="button"
          onClick={() => setShowArchive(!showArchive)}
          className="w-full flex items-center gap-base text-sm text-low hover:text-normal transition-colors duration-100"
        >
          {showArchive ? (
            <>
              <ArrowLeftIcon className="size-icon-xs" />
              <span>Back to Active</span>
            </>
          ) : (
            <>
              <ArchiveIcon className="size-icon-xs" />
              <span>View Archive</span>
              {archivedWorkspaces.length > 0 && (
                <span className="ml-auto text-xs bg-tertiary px-1.5 py-0.5 rounded">
                  {archivedWorkspaces.length}
                </span>
              )}
            </>
          )}
        </button>
      </div>
    </div>
  );
}
