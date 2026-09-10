import { useState } from 'react';
import { ArrowSquareOutIcon, SpinnerIcon } from '@phosphor-icons/react';
import { useTranslation } from 'react-i18next';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { importApi } from '@/shared/lib/api';
import { cn } from '@/shared/lib/utils';
import type { Task } from 'shared/types';

export function ImportedIssueList({
  tasks,
  isLoading,
}: {
  tasks: Task[];
  isLoading?: boolean;
}) {
  const { t } = useTranslation('common');

  if (isLoading) {
    return (
      <div className="flex items-center gap-half text-sm text-low">
        <SpinnerIcon className="size-icon-sm animate-spin" />
        {t('import.page.loadingIssues')}
      </div>
    );
  }

  if (tasks.length === 0) {
    return <p className="text-sm text-low">{t('import.page.noIssues')}</p>;
  }

  return (
    <ul className="space-y-half">
      {tasks.map((task) => (
        <ImportedIssueRow key={task.id} task={task} />
      ))}
    </ul>
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
      setError(err instanceof Error ? err.message : t('import.page.openError'));
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
