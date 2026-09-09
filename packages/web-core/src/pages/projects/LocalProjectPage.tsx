import { useMemo, useState } from 'react';
import {
  GitBranchIcon,
  PencilSimpleIcon,
  PlusIcon,
  SquaresFourIcon,
  TrashIcon,
} from '@phosphor-icons/react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import type { Repo } from 'shared/types';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { useCurrentAppDestination } from '@/shared/hooks/useCurrentAppDestination';
import {
  localProjectKeys,
  useLocalProject,
} from '@/shared/hooks/useLocalProjects';
import { projectApi, repoApi } from '@/shared/lib/api';
import { formatRelativeTime } from '@/shared/lib/date';
import { projectColorFromId } from '@/shared/lib/colors';
import { getProjectDestination } from '@/shared/lib/routes/appNavigation';
import { setCreateModeSeedState } from '@/features/create-mode/model/createModeSeedStore';
import { ConfirmDialog } from '@/shared/dialogs/shared/ConfirmDialog';
import { FolderPickerDialog } from '@/shared/dialogs/shared/FolderPickerDialog';
import {
  SelectionDialog,
  type SelectionPage,
} from '@/shared/dialogs/command-bar/SelectionDialog';
import {
  buildRepoSelectionPages,
  type RepoSelectionResult,
} from '@/shared/dialogs/command-bar/selections/repoSelection';
import { cn } from '@/shared/lib/utils';

function repoLabel(repo: Repo): string {
  return repo.display_name || repo.name;
}

export function LocalProjectPage() {
  const { t } = useTranslation('common');
  const appNavigation = useAppNavigation();
  const destination = useCurrentAppDestination();
  const projectId = getProjectDestination(destination)?.projectId ?? null;
  const queryClient = useQueryClient();
  const { data: project, isLoading, isError } = useLocalProject(projectId);
  const [error, setError] = useState<string | null>(null);
  const [isRenaming, setIsRenaming] = useState(false);
  const [draftName, setDraftName] = useState('');

  const attachedRepoIds = useMemo(
    () => new Set(project?.repos.map((repo) => repo.id) ?? []),
    [project]
  );

  const invalidate = async () => {
    if (!projectId) return;
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: localProjectKeys.all }),
      queryClient.invalidateQueries({
        queryKey: localProjectKeys.detail(projectId),
      }),
    ]);
  };

  const attachRepo = useMutation({
    mutationFn: async (repoId: string) => {
      if (!projectId) throw new Error('Missing project');
      return projectApi.attachRepo(projectId, { repo_id: repoId });
    },
    onSuccess: invalidate,
  });

  const detachRepo = useMutation({
    mutationFn: async (repoId: string) => {
      if (!projectId) throw new Error('Missing project');
      return projectApi.detachRepo(projectId, repoId);
    },
    onSuccess: invalidate,
  });

  const renameProject = useMutation({
    mutationFn: async (name: string) => {
      if (!projectId) throw new Error('Missing project');
      return projectApi.update(projectId, { name });
    },
    onSuccess: invalidate,
  });

  const deleteProject = useMutation({
    mutationFn: async () => {
      if (!projectId) throw new Error('Missing project');
      return projectApi.delete(projectId);
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: localProjectKeys.all });
      appNavigation.goToProjectsOverview();
    },
  });

  const handleChooseExistingRepo = async () => {
    setError(null);
    try {
      const repos = (await repoApi.list()).filter(
        (repo) => !attachedRepoIds.has(repo.id)
      );
      if (repos.length === 0) {
        setError(t('lite.projects.noAvailableRepos'));
        return;
      }
      const result = (await SelectionDialog.show({
        initialPageId: 'selectRepo',
        pages: buildRepoSelectionPages(
          repos.map((repo) => ({
            id: repo.id,
            display_name: repoLabel(repo),
          }))
        ) as Record<string, SelectionPage>,
      })) as RepoSelectionResult | undefined;
      if (!result?.repoId) return;
      await attachRepo.mutateAsync(result.repoId);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : t('lite.projects.attachRepoError')
      );
    }
  };

  const handleBrowseRepo = async () => {
    setError(null);
    try {
      const selectedPath = await FolderPickerDialog.show({
        title: t('lite.projects.browseRepoTitle'),
        description: t('lite.projects.browseRepoDescription'),
      });
      if (!selectedPath) return;
      const repo = await repoApi.register({
        path: selectedPath,
        allow_non_git: true,
      });
      await attachRepo.mutateAsync(repo.id);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : t('lite.projects.attachRepoError')
      );
    }
  };

  const handleNewWorkspace = () => {
    if (!project) return;
    setCreateModeSeedState({
      project_id: project.id,
      preferredRepos: project.repos.map((repo) => ({
        repo_id: repo.id,
        target_branch: repo.default_target_branch,
      })),
    });
    appNavigation.goToWorkspacesCreate();
  };

  const startRename = () => {
    if (!project) return;
    setDraftName(project.name);
    setIsRenaming(true);
    setError(null);
  };

  const handleRename = async () => {
    const trimmed = draftName.trim();
    if (trimmed.length < 2) {
      setError(t('lite.projects.nameTooShort'));
      return;
    }
    setError(null);
    try {
      await renameProject.mutateAsync(trimmed);
      setIsRenaming(false);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : t('lite.projects.renameError')
      );
    }
  };

  const handleDelete = async () => {
    if (!project) return;
    const result = await ConfirmDialog.show({
      title: t('lite.projects.deleteTitle'),
      message: t('lite.projects.deleteMessage', { name: project.name }),
      confirmText: t('lite.projects.deleteConfirm'),
      cancelText: t('lite.projects.cancel'),
      variant: 'destructive',
    });
    if (result !== 'confirmed') return;
    setError(null);
    try {
      await deleteProject.mutateAsync();
    } catch (err) {
      setError(
        err instanceof Error ? err.message : t('lite.projects.deleteError')
      );
    }
  };

  if (!projectId) {
    return null;
  }

  if (isLoading) {
    return (
      <div className="h-full overflow-auto bg-primary">
        <div className="mx-auto w-full max-w-4xl px-base py-double">
          <div className="h-48 animate-pulse rounded-md border border-border bg-secondary" />
        </div>
      </div>
    );
  }

  if (isError || !project) {
    return (
      <div className="h-full overflow-auto bg-primary">
        <div className="mx-auto w-full max-w-4xl px-base py-double">
          <button
            type="button"
            onClick={() => appNavigation.goToProjectsOverview()}
            className="inline-flex items-center gap-half text-sm text-low hover:text-normal"
          >
            <SquaresFourIcon className="size-icon-sm" weight="bold" />
            {t('lite.projects.backToOverview')}
          </button>
          <p className="mt-base text-sm text-low">
            {t('lite.projects.notFound')}
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full overflow-auto bg-primary">
      <div className="mx-auto w-full max-w-4xl px-base py-base sm:px-double sm:py-double">
        <button
          type="button"
          onClick={() => appNavigation.goToProjectsOverview()}
          className="inline-flex items-center gap-half text-sm text-low hover:text-normal"
        >
          <SquaresFourIcon className="size-icon-sm" weight="bold" />
          {t('lite.projects.backToOverview')}
        </button>

        <header className="mt-base space-y-half">
          <div className="flex flex-wrap items-center gap-half">
            <span
              className="size-3 rounded-full"
              style={{
                backgroundColor: `hsl(${projectColorFromId(project.id)})`,
              }}
            />
            {isRenaming ? (
              <form
                className="flex min-w-0 flex-1 flex-wrap items-center gap-half"
                onSubmit={(event) => {
                  event.preventDefault();
                  void handleRename();
                }}
              >
                <input
                  value={draftName}
                  onChange={(event) => setDraftName(event.target.value)}
                  className="min-w-0 flex-1 rounded border border-border bg-secondary px-base py-half text-lg font-semibold text-high focus:outline-none focus:ring-1 focus:ring-brand"
                  autoFocus
                />
                <button
                  type="submit"
                  disabled={renameProject.isPending}
                  className="rounded border border-brand/50 px-base py-half text-sm font-medium text-brand hover:border-brand"
                >
                  {renameProject.isPending
                    ? t('lite.projects.saving')
                    : t('lite.projects.saveName')}
                </button>
                <button
                  type="button"
                  onClick={() => setIsRenaming(false)}
                  className="rounded border border-border px-base py-half text-sm text-normal hover:bg-panel"
                >
                  {t('lite.projects.cancel')}
                </button>
              </form>
            ) : (
              <>
                <h1 className="min-w-0 truncate text-2xl font-semibold text-high">
                  {project.name}
                </h1>
                <button
                  type="button"
                  onClick={startRename}
                  className="rounded p-half text-low hover:bg-panel hover:text-normal"
                  aria-label={t('lite.projects.rename')}
                >
                  <PencilSimpleIcon className="size-icon-sm" />
                </button>
                <button
                  type="button"
                  onClick={() => void handleDelete()}
                  disabled={deleteProject.isPending}
                  className="rounded p-half text-low hover:bg-panel hover:text-error"
                  aria-label={t('lite.projects.delete')}
                >
                  <TrashIcon className="size-icon-sm" />
                </button>
              </>
            )}
          </div>
          <p className="text-sm text-low">
            {t('lite.projects.detailSubtitle')}
          </p>
        </header>

        {error && (
          <p className="mt-base rounded border border-error/40 bg-error/10 px-base py-half text-sm text-error">
            {error}
          </p>
        )}

        <section className="mt-double space-y-base">
          <div className="flex flex-wrap items-center justify-between gap-base">
            <h2 className="text-lg font-medium text-high">
              {t('lite.projects.reposHeading')}
            </h2>
            <div className="flex flex-wrap gap-half">
              <button
                type="button"
                onClick={() => void handleChooseExistingRepo()}
                className="inline-flex items-center gap-half rounded border border-border bg-secondary px-base py-half text-sm text-normal hover:bg-panel"
              >
                {t('lite.projects.chooseExisting')}
              </button>
              <button
                type="button"
                onClick={() => void handleBrowseRepo()}
                className="inline-flex items-center gap-half rounded border border-brand/50 bg-secondary px-base py-half text-sm font-medium text-brand hover:border-brand"
              >
                <PlusIcon className="size-icon-sm" weight="bold" />
                {t('lite.projects.attachRepo')}
              </button>
            </div>
          </div>

          {project.repos.length === 0 ? (
            <p className="rounded border border-border bg-secondary p-base text-sm text-low">
              {t('lite.projects.noRepos')}
            </p>
          ) : (
            <ul className="space-y-half">
              {project.repos.map((repo) => (
                <li
                  key={repo.id}
                  className="flex items-center justify-between gap-base rounded border border-border bg-secondary px-base py-half"
                >
                  <div className="min-w-0">
                    <p className="truncate text-sm font-medium text-high">
                      {repoLabel(repo)}
                    </p>
                    <p className="truncate text-xs text-low">{repo.path}</p>
                  </div>
                  <button
                    type="button"
                    onClick={() => void detachRepo.mutateAsync(repo.id)}
                    className="rounded p-half text-low hover:bg-panel hover:text-error"
                    aria-label={t('lite.projects.detachRepo')}
                  >
                    <TrashIcon className="size-icon-sm" />
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>

        <section className="mt-double space-y-base">
          <div className="flex flex-wrap items-center justify-between gap-base">
            <h2 className="text-lg font-medium text-high">
              {t('lite.projects.workspacesHeading')}
            </h2>
            <button
              type="button"
              onClick={handleNewWorkspace}
              className="inline-flex items-center gap-half rounded border border-brand/50 bg-secondary px-base py-half text-sm font-medium text-brand hover:border-brand"
            >
              <PlusIcon className="size-icon-sm" weight="bold" />
              {t('lite.projects.newWorkspace')}
            </button>
          </div>

          {project.workspaces.length === 0 ? (
            <p className="rounded border border-border bg-secondary p-base text-sm text-low">
              {t('lite.projects.noWorkspaces')}
            </p>
          ) : (
            <ul className="space-y-half">
              {project.workspaces.map((workspace) => (
                <li key={workspace.id}>
                  <button
                    type="button"
                    onClick={() => appNavigation.goToWorkspace(workspace.id)}
                    className={cn(
                      'flex w-full items-center justify-between gap-base rounded border border-border bg-secondary px-base py-half text-left hover:bg-panel'
                    )}
                  >
                    <span className="min-w-0">
                      <span className="block truncate text-sm text-high">
                        {workspace.name || t('lite.projects.unnamedWorkspace')}
                      </span>
                      <span className="flex items-center gap-half text-xs text-low">
                        <GitBranchIcon className="size-3" />
                        {workspace.branch}
                      </span>
                    </span>
                    <span className="shrink-0 text-xs text-low">
                      {formatRelativeTime(workspace.updated_at)}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>
    </div>
  );
}
