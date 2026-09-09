import { useMemo, useState } from 'react';
import {
  FolderSimpleIcon,
  GitBranchIcon,
  MagnifyingGlassIcon,
  PlusIcon,
  SquaresFourIcon,
} from '@phosphor-icons/react';
import { useTranslation } from 'react-i18next';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { useLocalProjects } from '@/shared/hooks/useLocalProjects';
import {
  CreateLocalProjectDialog,
  type CreateLocalProjectResult,
} from '@/shared/dialogs/org/CreateLocalProjectDialog';
import { formatRelativeTime } from '@/shared/lib/date';
import { projectColorFromId } from '@/shared/lib/colors';

export function LocalProjectsOverviewPage() {
  const { t } = useTranslation('common');
  const appNavigation = useAppNavigation();
  const [search, setSearch] = useState('');
  const { data: projects = [], isLoading } = useLocalProjects();

  const cards = useMemo(() => {
    const query = search.trim().toLowerCase();
    return projects.filter((project) => {
      if (!query) return true;
      return (
        project.name.toLowerCase().includes(query) ||
        project.repos.some(
          (repo) =>
            repo.display_name.toLowerCase().includes(query) ||
            repo.name.toLowerCase().includes(query) ||
            repo.path.toLowerCase().includes(query)
        )
      );
    });
  }, [projects, search]);

  const handleCreate = async () => {
    const result: CreateLocalProjectResult =
      await CreateLocalProjectDialog.show();
    if (result.action === 'created' && result.project) {
      appNavigation.goToProject(result.project.id);
    }
  };

  return (
    <div className="h-full overflow-auto bg-primary">
      <div className="mx-auto w-full max-w-6xl px-base py-base sm:px-double sm:py-double">
        <header className="space-y-half">
          <div className="flex items-center gap-half text-low">
            <SquaresFourIcon className="size-icon-base" weight="bold" />
            <span className="text-sm">{t('lite.projects.kicker')}</span>
          </div>
          <h1 className="text-2xl font-semibold text-high">
            {t('lite.projects.title')}
          </h1>
          <p className="text-sm text-low">{t('lite.projects.subtitle')}</p>
        </header>

        <div className="mt-double flex flex-col gap-base sm:flex-row sm:items-center">
          <label className="relative min-w-0 flex-1">
            <MagnifyingGlassIcon className="pointer-events-none absolute left-base top-1/2 size-icon-base -translate-y-1/2 text-low" />
            <input
              type="search"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              placeholder={t('lite.projects.searchPlaceholder')}
              className="w-full rounded border border-border bg-secondary py-half pl-8 pr-base text-base text-normal placeholder:text-low focus:outline-none focus:ring-1 focus:ring-brand"
            />
          </label>
          <button
            type="button"
            onClick={() => appNavigation.goToWorkspaces()}
            className="inline-flex items-center gap-half rounded border border-border bg-secondary px-base py-half text-sm font-medium text-normal hover:border-high/20 hover:bg-panel"
          >
            {t('lite.projects.viewRepos')}
          </button>
          <button
            type="button"
            onClick={() => void handleCreate()}
            className="inline-flex items-center gap-half rounded border border-brand/50 bg-secondary px-base py-half text-sm font-medium text-brand hover:border-brand hover:bg-panel"
          >
            <PlusIcon className="size-icon-sm" weight="bold" />
            {t('lite.projects.new')}
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
              {t('lite.projects.emptyTitle')}
            </h2>
            <p className="mt-half text-sm text-low">
              {t('lite.projects.emptyBody')}
            </p>
            <button
              type="button"
              onClick={() => void handleCreate()}
              className="mt-base inline-flex items-center gap-half rounded border border-brand/50 px-base py-half text-sm font-medium text-brand hover:border-brand"
            >
              <PlusIcon className="size-icon-sm" weight="bold" />
              {t('lite.projects.new')}
            </button>
          </section>
        ) : (
          <ul className="mt-double grid gap-base lg:grid-cols-2">
            {cards.map((project) => (
              <li key={project.id}>
                <button
                  type="button"
                  onClick={() => appNavigation.goToProject(project.id)}
                  className="flex h-full w-full flex-col rounded-md border border-border bg-secondary p-base text-left hover:border-high/20 hover:bg-panel"
                >
                  <div className="flex items-start justify-between gap-base">
                    <div className="min-w-0">
                      <div className="flex items-center gap-half">
                        <span
                          className="size-2.5 shrink-0 rounded-full"
                          style={{
                            backgroundColor: `hsl(${projectColorFromId(project.id)})`,
                          }}
                        />
                        <h2 className="truncate text-base font-medium text-high">
                          {project.name}
                        </h2>
                      </div>
                      <p className="mt-half text-sm text-low">
                        {t('lite.projects.repoCount', {
                          count: project.repos.length,
                        })}
                        {' · '}
                        {t('lite.projects.workspaceCount', {
                          count: project.workspace_count,
                        })}
                      </p>
                    </div>
                    <span className="shrink-0 text-xs text-low">
                      {formatRelativeTime(project.updated_at)}
                    </span>
                  </div>
                  {project.repos.length === 0 ? (
                    <p className="mt-base text-sm text-low">
                      {t('lite.projects.noRepos')}
                    </p>
                  ) : (
                    <ul className="mt-base space-y-half">
                      {project.repos.slice(0, 3).map((repo) => (
                        <li
                          key={repo.id}
                          className="flex items-center gap-half text-sm text-normal"
                        >
                          <FolderSimpleIcon className="size-3 shrink-0 text-low" />
                          <span className="truncate">
                            {repo.display_name || repo.name}
                          </span>
                        </li>
                      ))}
                      {project.repos.length > 3 && (
                        <li className="flex items-center gap-half text-xs text-low">
                          <GitBranchIcon className="size-3" />+
                          {project.repos.length - 3}
                        </li>
                      )}
                    </ul>
                  )}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
