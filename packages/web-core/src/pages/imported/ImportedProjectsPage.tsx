import { useState } from 'react';
import {
  CaretDownIcon,
  CaretRightIcon,
  SquaresFourIcon,
  TrayArrowDownIcon,
} from '@phosphor-icons/react';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  IMPORTED_PROJECTS_QUERY_KEY,
  ImportWebExportCard,
} from '@/shared/components/ImportWebExportCard';
import { ImportedIssueList } from '@/shared/components/ImportedIssueList';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { useUserSystem } from '@/shared/hooks/useUserSystem';
import { importApi } from '@/shared/lib/api';
import type { ImportedProjectSummary } from 'shared/types';

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
  const { liteMode } = useUserSystem();
  const appNavigation = useAppNavigation();
  const [open, setOpen] = useState(false);
  const { data, isFetching } = useQuery({
    queryKey: [...IMPORTED_PROJECTS_QUERY_KEY, project.id],
    queryFn: () => importApi.getImportedProject(project.id),
    enabled: open,
  });

  return (
    <li className="rounded-md border border-border bg-secondary">
      <div className="flex items-center gap-half px-base py-base">
        <button
          type="button"
          onClick={() => setOpen((value) => !value)}
          className="flex min-w-0 flex-1 items-center justify-between gap-base text-left"
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
        {liteMode && (
          <button
            type="button"
            onClick={() => appNavigation.goToProject(project.id)}
            className="inline-flex shrink-0 items-center gap-half rounded border border-border px-half py-half text-xs font-medium text-normal hover:border-brand hover:bg-panel"
          >
            <SquaresFourIcon className="size-3" />
            {t('import.page.openProject')}
          </button>
        )}
      </div>

      {open && (
        <div className="border-t border-border px-base py-base">
          <ImportedIssueList
            tasks={data?.tasks ?? []}
            isLoading={isFetching && !data}
          />
        </div>
      )}
    </li>
  );
}
