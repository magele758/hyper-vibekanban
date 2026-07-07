import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import type { ReadFileResponse } from 'shared/types';
import { SpinnerIcon } from '@phosphor-icons/react';
import WYSIWYGEditor from '@/shared/components/WYSIWYGEditor';
import { MarkdownPreview } from '@/shared/components/MarkdownPreview';
import { workspacesApi } from '@/shared/lib/api';
import { useWorkspaceContext } from '@/shared/hooks/useWorkspaceContext';
import { useTheme } from '@/shared/hooks/useTheme';
import { getActualTheme } from '@/shared/lib/theme';
import { cn } from '@/shared/lib/utils';

const SAVE_DEBOUNCE_MS = 600;

interface MarkdownFilePreviewContainerProps {
  /** Repo-relative path of the markdown file. */
  path: string;
  /** Optional repo to resolve the path against. */
  repoId?: string;
  className?: string;
}

/**
 * Heuristic: does the file contain markdown the WYSIWYG editor cannot represent
 * losslessly? The editor only knows the standard @lexical/markdown transformers
 * (plus tables/images/mermaid) — it has no node for YAML frontmatter, raw HTML,
 * or footnotes. Round-tripping such a file through the editor silently drops or
 * mangles that content, so we render those files read-only instead of risking a
 * destructive auto-save.
 *
 * This is deliberately conservative: a false positive only makes a file
 * read-only (safe), whereas a false negative could corrupt it. HTML inside a
 * fenced code block will also trip this, which is an acceptable trade-off.
 */
function hasUneditableMarkdownSyntax(content: string): boolean {
  // YAML frontmatter block at the very start of the file.
  if (/^﻿?\s*---\r?\n[\s\S]*?\r?\n---\s*(\r?\n|$)/.test(content)) {
    return true;
  }
  // HTML comments.
  if (content.includes('<!--')) return true;
  // Footnote references / definitions (e.g. [^1]).
  if (/\[\^[^\]]+\]/.test(content)) return true;
  // Raw HTML tags (block or inline). Requires a tag name after '<' followed by
  // attributes/whitespace/'>', so autolinks like <https://…> do not match.
  if (/<\/?[a-zA-Z][a-zA-Z0-9-]*(\s[^>]*)?\/?>/.test(content)) return true;
  return false;
}

/**
 * Fetches a markdown file from the workspace worktree and renders it in an
 * editable WYSIWYG editor (same component used by the conversation stream).
 * Edits are debounce-saved back to the file on disk.
 */
export function MarkdownFilePreviewContainer({
  path,
  repoId,
  className,
}: MarkdownFilePreviewContainerProps) {
  const { t } = useTranslation('common');
  const { workspaceId } = useWorkspaceContext();
  const { theme } = useTheme();
  const actualTheme = getActualTheme(theme);
  const queryClient = useQueryClient();

  const queryKey = ['workspaceFile', workspaceId, path, repoId] as const;
  const { data, isLoading, error } = useQuery({
    queryKey,
    queryFn: () => workspacesApi.readFile(workspaceId!, path, repoId),
    enabled: !!workspaceId && !!path,
    staleTime: 30_000,
    // Never refetch behind the user's back — a refocus/remount refetch would
    // overwrite in-progress edits with the (reflowed) on-disk version.
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
  });

  // Files containing syntax the WYSIWYG editor can't round-trip losslessly
  // (frontmatter, raw HTML, footnotes) are rendered read-only to avoid a
  // destructive auto-save. This is decided from the on-disk content.
  const isReadOnly = useMemo(
    () => (data ? hasUneditableMarkdownSyntax(data.content) : false),
    [data]
  );

  // Local editing state for immediate feedback; the resolved repo id from the
  // read response is what we write back to (avoids multi-repo ambiguity).
  const [localContent, setLocalContent] = useState('');
  const [saveStatus, setSaveStatus] = useState<'idle' | 'saved'>('idle');
  const isEditingRef = useRef(false);
  // Only real user input may trigger a save. The editor emits an onChange right
  // after the initial parse (MarkdownSyncPlugin reflows the input to Lexical's
  // canonical form), which is programmatic — treating it as an edit would
  // rewrite the file just from opening it. We flip this true on actual DOM
  // input events (typing/paste/etc.), which programmatic reflows never fire.
  const userInteractedRef = useRef(false);
  const resolvedRepoIdRef = useRef<string | undefined>(repoId);

  // Save coordination: `pendingContentRef` holds the latest unsaved content;
  // `savingRef` serializes writes so only one PUT is in flight at a time and
  // the most recent content always wins (no out-of-order overwrite). The debounce
  // timer is a plain ref so we can flush it synchronously on unmount.
  const pendingContentRef = useRef<string | null>(null);
  const savingRef = useRef(false);
  const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Keep the values the writer needs in refs so the flush path (which must run
  // at unmount, outside render) always sees current values without re-binding.
  const writeCtxRef = useRef({ workspaceId, path, queryClient, queryKey });
  writeCtxRef.current = { workspaceId, path, queryClient, queryKey };

  // Drain `pendingContentRef` to disk, one write at a time. Re-runs itself if
  // more content arrived while a write was in flight, so the final state wins.
  const flushSave = useCallback(async () => {
    if (savingRef.current) return;
    const content = pendingContentRef.current;
    const targetRepoId = resolvedRepoIdRef.current;
    const { workspaceId, path, queryClient, queryKey } = writeCtxRef.current;
    if (content === null || !workspaceId || !targetRepoId) return;

    savingRef.current = true;
    pendingContentRef.current = null;
    try {
      await workspacesApi.writeFile(workspaceId, {
        path,
        content,
        repo_id: targetRepoId,
      });
      // Keep the query cache in sync with exactly what we wrote, so any later
      // read returns the saved content rather than a stale/reflowed version.
      queryClient.setQueryData<ReadFileResponse>(queryKey, (prev) =>
        prev ? { ...prev, content } : prev
      );
      setSaveStatus('saved');
    } catch (e) {
      console.error('Failed to save markdown file', e);
    } finally {
      savingRef.current = false;
      isEditingRef.current = false;
      // Content changed again while we were writing — persist the newest.
      if (pendingContentRef.current !== null) {
        void flushSave();
      }
    }
  }, []);

  // Sync from server when the file loads (but not while the user is editing).
  useEffect(() => {
    if (isLoading) return;
    if (isEditingRef.current) return;
    if (data) {
      setLocalContent(data.content);
      resolvedRepoIdRef.current = data.repo_id;
    }
  }, [isLoading, data]);

  // On unmount (including switching to another file, which remounts via key),
  // cancel the pending debounce and flush any unsaved content so edits are not
  // lost. Fire-and-forget: the component is gone, so no state updates here.
  useEffect(() => {
    return () => {
      if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
      if (pendingContentRef.current !== null) {
        void flushSave();
      }
    };
  }, [flushSave]);

  const handleChange = useCallback(
    (content: string) => {
      // Only persist changes that follow real user input. The initial reflow
      // (programmatic) arrives before any DOM input event, so it's ignored —
      // this prevents rewriting the file just from opening it.
      if (!userInteractedRef.current) return;
      if (content === localContent) return;

      isEditingRef.current = true;
      setLocalContent(content);
      setSaveStatus('idle');
      pendingContentRef.current = content;

      if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
      debounceTimerRef.current = setTimeout(() => {
        debounceTimerRef.current = null;
        void flushSave();
      }, SAVE_DEBOUNCE_MS);
    },
    [flushSave, localContent]
  );

  const markUserInteracted = useCallback(() => {
    userInteractedRef.current = true;
  }, []);

  const fileName = path.split('/').pop() ?? path;

  return (
    <div className={cn('h-full bg-secondary flex flex-col', className)}>
      <div className="px-4 py-2 border-b border-border shrink-0 flex items-center gap-2">
        <span className="text-sm font-medium text-normal truncate" title={path}>
          {fileName}
        </span>
        {data && isReadOnly && (
          <span
            className="text-xs text-low border border-border rounded px-1.5 py-0.5 shrink-0"
            title={t('markdownPreview.readOnlyReason')}
          >
            {t('markdownPreview.readOnlyBadge')}
          </span>
        )}
      </div>
      <div className="flex-1 min-h-0 overflow-y-auto p-4">
        {isLoading ? (
          <div className="flex items-center justify-center h-full">
            <SpinnerIcon className="animate-spin h-5 w-5 text-low" />
          </div>
        ) : error || !data ? (
          <div className="flex items-center justify-center h-full text-low text-sm">
            {t('markdownPreview.loadError', { path })}
          </div>
        ) : isReadOnly ? (
          <MarkdownPreview content={data.content} theme={actualTheme} />
        ) : (
          <div
            onInput={markUserInteracted}
            onKeyDown={markUserInteracted}
            onPaste={markUserInteracted}
          >
            <WYSIWYGEditor
              value={localContent}
              onChange={handleChange}
              workspaceId={workspaceId}
              className="min-h-[300px]"
              showStaticToolbar
              saveStatus={saveStatus}
            />
          </div>
        )}
      </div>
    </div>
  );
}
