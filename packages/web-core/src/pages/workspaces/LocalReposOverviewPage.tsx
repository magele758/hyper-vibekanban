import { useMemo, useState } from 'react';
import {
  FolderSimpleIcon,
  GitBranchIcon,
  MagnifyingGlassIcon,
  PlusIcon,
} from '@phosphor-icons/react';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { useWorkspaceRepoLinks } from '@/shared/hooks/useWorkspaceRepoLinks';
import { useWorkspaces } from '@/shared/hooks/useWorkspaces';
import { repoApi } from '@/shared/lib/api';
import { formatRelativeTime } from '@/shared/lib/date';
import { cn } from '@/shared/lib/utils';
import type { Repo } from 'shared/types';

function repoLabel(repo: Repo): string {
  return repo.display_name || repo.name;
}

export function LocalReposOverviewPage() {
  const { t } = useTranslation('common');
  const appNavigation = useAppNavigation();
  const [search, setSearch] = useState('');
  const { data: repos = [], isLoading: reposLoading } = useQuery({
    queryKey: ['lite-repos'],
    queryFn: () => repoApi.list(),
  });
  const {
    workspaces,
    archivedWorkspaces,
    isLoading: workspacesLoading,
  } = useWorkspaces();
  const { workspaceIdsByRepo, isLoading: linksLoading } =
    useWorkspaceRepoLinks();

  const allWorkspaces = useMemo(
    () => [...workspaces, ...archivedWorkspaces],
    [workspaces, archivedWorkspaces]
  );

  const cards = useMemo(() => {
    const query = search.trim().toLowerCase();
    return repos
      .map((repo) => {
        const workspaceIds = new Set(workspaceIdsByRepo.get(repo.id) ?? []);
        const repoWorkspaces = allWorkspaces
          .filter((workspace) => workspaceIds.has(workspace.id))
          .sort(
            (a, b) =>
              new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime()
          );
        return { repo, repoWorkspaces };
      })
      .filter(({ repo, repoWorkspaces }) => {
        if (!query) return true;
        return (
          repoLabel(repo).toLowerCase().includes(query) ||
          repo.path.toLowerCase().includes(query) ||
          repoWorkspaces.some(
            (workspace) =>
              workspace.name.toLowerCase().includes(query) ||
              workspace.branch.toLowerCase().includes(query)
          )
        );
      })
      .sort((a, b) => {
        const aUpdated = a.repoWorkspaces[0]?.updatedAt ?? a.repo.updated_at;
        const bUpdated = b.repoWorkspaces[0]?.updatedAt ?? b.repo.updated_at;
        return new Date(bUpdated).getTime() - new Date(aUpdated).getTime();
      });
  }, [allWorkspaces, repos, search, workspaceIdsByRepo]);

  const isLoading = reposLoading || workspacesLoading || linksLoading;

  return (
    <div className="h-full overflow-auto bg-primary">
      <div className="mx-auto w-full max-w-6xl px-base py-base sm:px-double sm:py-double">
        <header className="space-y-half">
          <div className="flex items-center gap-half text-low">
            <FolderSimpleIcon className="size-icon-base" weight="bold" />
            <span className="text-sm">{t('lite.overview.kicker')}</span>
          </div>
          <h1 className="text-2xl font-semibold text-high">
            {t('lite.overview.title')}
          </h1>
          <p className="text-sm text-low">{t('lite.overview.subtitle')}</p>
        </header>

        <div className="mt-double flex flex-col gap-base sm:flex-row sm:items-center">
          <label className="relative min-w-0 flex-1">
            <MagnifyingGlassIcon className="pointer-events-none absolute left-base top-1/2 size-icon-base -translate-y-1/2 text-low" />
            <input
              type="search"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              placeholder={t('lite.overview.searchPlaceholder')}
              className="w-full rounded border border-border bg-secondary py-half pl-8 pr-base text-base text-normal placeholder:text-low focus:outline-none focus:ring-1 focus:ring-brand"
            />
          </label>
          <button
            type="button"
            onClick={() => appNavigation.goToWorkspacesCreate()}
            className="inline-flex items-center gap-half rounded border border-brand/50 bg-secondary px-base py-half text-sm font-medium text-brand hover:border-brand hover:bg-panel"
          >
            <PlusIcon className="size-icon-sm" weight="bold" />
            {t('lite.overview.newWorkspace')}
          </button>
        </div>

        {isLoading ? (
          <div className="mt-double grid gap-base sm:grid-cols-2">
            {Array.from({ length: 4 }).map((_, index) => (
              <div
                key={index}
                className="h-40 animate-pulse rounded-md border border-border bg-secondary"
              />
            ))}
          </div>
        ) : cards.length === 0 ? (
          <section className="mt-double rounded-sm border border-border bg-secondary p-base sm:p-double">
            <h2 className="text-base font-medium text-high">
              {t('lite.overview.emptyTitle')}
            </h2>
            <p className="mt-half text-sm text-low">
              {t('lite.overview.emptyBody')}
            </p>
            <button
              type="button"
              onClick={() => appNavigation.goToWorkspacesCreate()}
              className="mt-base inline-flex items-center gap-half rounded border border-brand/50 px-base py-half text-sm font-medium text-brand hover:border-brand"
            >
              <PlusIcon className="size-icon-sm" weight="bold" />
              {t('lite.overview.newWorkspace')}
            </button>
          </section>
        ) : (
          <ul className="mt-double grid gap-base lg:grid-cols-2">
            {cards.map(({ repo, repoWorkspaces }) => (
              <li
                key={repo.id}
                className="rounded-md border border-border bg-secondary p-base"
              >
                <div className="flex items-start justify-between gap-base">
                  <div className="min-w-0">
                    <h2 className="truncate text-base font-medium text-high">
                      {repoLabel(repo)}
                    </h2>
                    <p className="mt-half truncate text-sm text-low">
                      {repo.path}
                    </p>
                  </div>
                  <span className="shrink-0 text-sm text-low">
                    {t('lite.overview.workspaceCount', {
                      count: repoWorkspaces.length,
                    })}
                  </span>
                </div>

                {repoWorkspaces.length === 0 ? (
                  <p className="mt-base text-sm text-low">
                    {t('lite.overview.noWorkspaces')}
                  </p>
                ) : (
                  <ul className="mt-base space-y-half">
                    {repoWorkspaces.slice(0, 5).map((workspace) => (
                      <li key={workspace.id}>
                        <button
                          type="button"
                          onClick={() =>
                            appNavigation.goToWorkspace(workspace.id)
                          }
                          className={cn(
                            'flex w-full items-center justify-between gap-base rounded border border-transparent px-half py-half text-left hover:border-border hover:bg-panel'
                          )}
                        >
                          <span className="min-w-0">
                            <span className="block truncate text-sm text-normal">
                              {workspace.name}
                            </span>
                            <span className="flex items-center gap-half text-xs text-low">
                              <GitBranchIcon className="size-3" />
                              {workspace.branch}
                            </span>
                          </span>
                          <span className="shrink-0 text-xs text-low">
                            {formatRelativeTime(workspace.updatedAt)}
                          </span>
                        </button>
                      </li>
                    ))}
                  </ul>
                )}
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
