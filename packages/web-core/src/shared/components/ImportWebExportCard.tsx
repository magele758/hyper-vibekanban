import { useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { FileArrowUpIcon, SpinnerIcon } from '@phosphor-icons/react';
import { ApiError, importApi } from '@/shared/lib/api';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { useUserSystem } from '@/shared/hooks/useUserSystem';
import { localProjectKeys } from '@/shared/hooks/useLocalProjects';
import { destinationAfterWebImport } from '@/shared/lib/importDestination';
import type { ImportWebExportResult } from 'shared/types';

export const IMPORTED_PROJECTS_QUERY_KEY = ['imported-projects'] as const;

type ImportWebExportCardProps = {
  showViewLink?: boolean;
  onImported?: () => void;
};

export function ImportWebExportCard({
  showViewLink = true,
  onImported,
}: ImportWebExportCardProps) {
  const { t } = useTranslation('common');
  const { liteMode } = useUserSystem();
  const appNavigation = useAppNavigation();
  const queryClient = useQueryClient();
  const inputRef = useRef<HTMLInputElement>(null);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ImportWebExportResult | null>(null);

  const handleFile = async (file: File | undefined) => {
    if (!file) {
      return;
    }
    setImporting(true);
    setError(null);
    setResult(null);
    try {
      const imported = await importApi.importWebExport(file);
      setResult(imported);
      await queryClient.invalidateQueries({
        queryKey: IMPORTED_PROJECTS_QUERY_KEY,
      });
      if (liteMode) {
        await Promise.all([
          queryClient.invalidateQueries({ queryKey: localProjectKeys.all }),
          ...imported.projects.map((project) =>
            queryClient.invalidateQueries({
              queryKey: localProjectKeys.detail(project.id),
            })
          ),
        ]);
        onImported?.();
        const destination = destinationAfterWebImport(imported);
        if (destination.kind === 'project') {
          appNavigation.goToProject(destination.projectId);
        } else {
          appNavigation.goToProjectsOverview();
        }
      }
    } catch (err) {
      const message =
        err instanceof ApiError ? err.message : t('import.settings.error');
      setError(message);
    } finally {
      setImporting(false);
      if (inputRef.current) {
        inputRef.current.value = '';
      }
    }
  };

  return (
    <div className="space-y-base">
      <input
        ref={inputRef}
        type="file"
        accept=".zip,.json,application/zip,application/json"
        className="hidden"
        onChange={(event) => {
          void handleFile(event.target.files?.[0]);
        }}
      />
      <button
        type="button"
        disabled={importing}
        onClick={() => inputRef.current?.click()}
        className="inline-flex items-center gap-half rounded border border-brand/50 bg-secondary px-base py-half text-sm font-medium text-brand hover:border-brand hover:bg-panel disabled:opacity-60"
      >
        {importing ? (
          <SpinnerIcon className="size-icon-sm animate-spin" weight="bold" />
        ) : (
          <FileArrowUpIcon className="size-icon-sm" weight="bold" />
        )}
        {importing
          ? t('import.settings.importing')
          : t('import.settings.chooseFile')}
      </button>

      {error && (
        <p className="text-sm text-error" role="alert">
          {error}
        </p>
      )}

      {result && (
        <p className="text-sm text-success">
          {t('import.settings.success', {
            projectsCreated: result.projects_created,
            projectsReused: result.projects_reused,
            issuesCreated: result.issues_created,
          })}
        </p>
      )}

      {showViewLink && (
        <button
          type="button"
          onClick={() =>
            liteMode
              ? appNavigation.goToProjectsOverview()
              : appNavigation.goToImported()
          }
          className="text-sm font-medium text-brand hover:underline"
        >
          {liteMode
            ? t('import.settings.viewInProjects')
            : t('import.settings.viewImported')}
        </button>
      )}
    </div>
  );
}
