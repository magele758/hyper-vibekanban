import { useState } from 'react';
import {
  ArrowSquareOutIcon,
  CaretDownIcon,
  CaretRightIcon,
  SpinnerIcon,
  TrayArrowDownIcon,
} from '@phosphor-icons/react';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  IMPORTED_PROJECTS_QUERY_KEY,
  ImportWebExportCard,
} from '@/shared/components/ImportWebExportCard';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { importApi } from '@/shared/lib/api';
import { cn } from '@/shared/lib/utils';
import type { ImportedProjectSummary, Task } from 'shared/types';

export function ImportedProjectsPage() {
  const { t } = useTranslation('common');
  const { data: projects = [], isLoading } = useQuery({
    queryKey: IMPORTED_PROJECTS_QUERY_KEY,
    queryFn: () => importApi.listImportedProjects(),
  });

  return (
    <div className="h-full overflow-auto bg-primary">
      <div className="mx-auto w-full max-w-6xl px-base py-base sm:px-double sm:py-double">
        <header className="space-y-half">
          <div className="flex items-center gap-half text-low">
            <TrayArrowDownIcon className="size-icon-base" weight="bold" />
            <span className="text-sm">{t('import.page.kicker')}</span>
          </div>
          <h1 className="text-2xl font-semibold text-high">
            {t('import.page.title')}
          </h1>
          <p className="text-sm text-low">{t('import.page.subtitle')}</p>
        </header>

        <section className="mt-double rounded-sm border border-border bg-secondary p-base">
          <h2 className="text-base font-medium text-high">
            {t('import.settings.title')}
          </h2>
          <p className="mt-half text-sm text-low">
            {t('import.settings.description')}
          </p>
          <div className="mt-base">
            <ImportWebExportCard showViewLink={false} />
          </div>
        </section>

        {isLoading ? (
          <div className="mt-double grid gap-base sm:grid-cols-2">
            {Array.from({ length: 4 }).map((_, index) => (
              <div
                key={index}
                className="h-32 animate-pulse rounded-md border border-border bg-secondary"
              />
            ))}
          </div>
        ) : projects.length === 0 ? (
          <section className="mt-double rounded-sm border border-border bg-secondary p-base sm:p-double">
            <h2 className="text-base font-medium text-high">
              {t('import.page.emptyTitle')}
            </h2>
            <p className="mt-half text-sm text-low">
              {t('import.page.emptyBody')}
            </p>
          </section>
        ) : (
          <ul className="mt-double space-y-base">
            {projects.map((project) => (
              <ImportedProjectCard key={project.id} project={project} />
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

function ImportedProjectCard({ project }: { project: ImportedProjectSummary }) {
  const { t } = useTranslation('common');
  const [open, setOpen] = useState(false);
  const { data, isFetching } = useQuery({
    queryKey: [...IMPORTED_PROJECTS_QUERY_KEY, project.id],
    queryFn: () => importApi.getImportedProject(project.id),
    enabled: open,
  });

  return (
    <li className="rounded-md border border-border bg-secondary">
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        className="flex w-full items-center justify-between gap-base px-base py-base text-left"
      >
        <span className="min-w-0">
          <span className="block truncate text-base font-medium text-high">
            {project.name}
          </span>
          <span className="text-sm text-low">
            {t('import.page.issueCount', { count: project.issue_count })}
          </span>
        </span>
        {open ? (
          <CaretDownIcon className="size-icon-base text-low" />
        ) : (
          <CaretRightIcon className="size-icon-base text-low" />
        )}
      </button>

      {open && (
        <div className="border-t border-border px-base py-base">
          {isFetching && !data ? (
            <div className="flex items-center gap-half text-sm text-low">
              <SpinnerIcon className="size-icon-sm animate-spin" />
              {t('import.page.loadingIssues')}
            </div>
          ) : !data || data.tasks.length === 0 ? (
            <p className="text-sm text-low">{t('import.page.noIssues')}</p>
          ) : (
            <ul className="space-y-half">
              {data.tasks.map((task) => (
                <ImportedIssueRow key={task.id} task={task} />
              ))}
            </ul>
          )}
        </div>
      )}
    </li>
  );
}

function ImportedIssueRow({ task }: { task: Task }) {
  const { t } = useTranslation('common');
  const appNavigation = useAppNavigation();
  const [opening, setOpening] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleOpen = async () => {
    setOpening(true);
    setError(null);
    try {
      const result = await importApi.createWorkspaceFromTask(task.id);
      appNavigation.goToWorkspace(result.workspace_id);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : t('import.page.openError')
      );
    } finally {
      setOpening(false);
    }
  };

  return (
    <li className="rounded border border-transparent px-half py-half hover:border-border hover:bg-panel">
      <div className="flex items-start justify-between gap-base">
        <div className="min-w-0">
          <p className="truncate text-sm text-normal">{task.title}</p>
          <p className="text-xs text-low">{task.status}</p>
          {task.description && (
            <p className="mt-half line-clamp-2 text-xs text-low">
              {task.description}
            </p>
          )}
          {error && (
            <p className="mt-half text-xs text-error" role="alert">
              {error}
            </p>
          )}
        </div>
        <button
          type="button"
          disabled={opening}
          onClick={() => {
            void handleOpen();
          }}
          className={cn(
            'inline-flex shrink-0 items-center gap-half rounded border border-brand/50 px-half py-half text-xs font-medium text-brand hover:border-brand disabled:opacity-60'
          )}
        >
          {opening ? (
            <SpinnerIcon className="size-3 animate-spin" />
          ) : (
            <ArrowSquareOutIcon className="size-3" />
          )}
          {opening
            ? t('import.page.opening')
            : task.parent_workspace_id
              ? t('import.page.openExisting')
              : t('import.page.openWorkspace')}
        </button>
      </div>
    </li>
  );
}
