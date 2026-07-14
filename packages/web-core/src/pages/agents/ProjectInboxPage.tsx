import { useMemo, useState } from 'react';
import { useParams } from '@tanstack/react-router';
import { TrayIcon, CheckIcon, ArchiveIcon } from '@phosphor-icons/react';
import { OrgProvider } from '@/shared/providers/remote/OrgProvider';
import { ProjectProvider } from '@/shared/providers/remote/ProjectProvider';
import { UserProvider } from '@/shared/providers/remote/UserProvider';
import { useOrganizationStore } from '@/shared/stores/useOrganizationStore';
import { useAuth } from '@/shared/hooks/auth/useAuth';
import { useUserContext } from '@/shared/hooks/useUserContext';
import { LoginRequiredPrompt } from '@/shared/dialogs/shared/LoginRequiredPrompt';
import { boardAgentsApi } from '@/shared/lib/boardAgentsApi';
import { workspacesApi } from '@/shared/lib/api';
import { cn } from '@/shared/lib/utils';
import type { InboxItem } from 'shared/remote-types';

function payloadString(
  payload: InboxItem['payload'],
  key: string
): string | undefined {
  if (!payload || typeof payload !== 'object' || Array.isArray(payload)) {
    return undefined;
  }
  const v = (payload as Record<string, unknown>)[key];
  return typeof v === 'string' ? v : undefined;
}

function InboxInner({ projectId }: { projectId: string }) {
  const { inboxItems, isLoading, error: syncError, retry } = useUserContext();
  const [showArchived, setShowArchived] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState<Set<string>>(new Set());

  const items = useMemo(() => {
    return inboxItems
      .filter((item) => !item.project_id || item.project_id === projectId)
      .filter((item) => showArchived || !item.archived_at)
      .slice()
      .sort(
        (a, b) =>
          new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
      );
  }, [inboxItems, projectId, showArchived]);

  const unreadCount = useMemo(
    () => items.filter((item) => !item.read_at && !item.archived_at).length,
    [items]
  );

  const withPending = async (id: string, fn: () => Promise<void>) => {
    setPending((prev) => new Set(prev).add(id));
    try {
      await fn();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setPending((prev) => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    }
  };

  const handleMarkRead = async (id: string) => {
    await withPending(id, () => boardAgentsApi.markInboxRead(id));
  };

  const handleArchive = async (id: string) => {
    await withPending(id, () => boardAgentsApi.archiveInboxItem(id));
  };

  const handleMergeApprove = async (item: InboxItem) => {
    const gateId = payloadString(item.payload, 'gate_id');
    const workspaceId = payloadString(item.payload, 'local_workspace_id');
    if (!gateId) {
      setError('缺少 gate_id，无法确认');
      return;
    }
    await withPending(item.id, async () => {
      if (workspaceId) {
        const repos = await workspacesApi.getRepos(workspaceId);
        for (const wr of repos) {
          await workspacesApi.merge(workspaceId, { repo_id: wr.id });
        }
      }
      await boardAgentsApi.respondPipelineGate(gateId, 'approve');
      await boardAgentsApi.markInboxRead(item.id);
    });
  };

  const handleMergeReject = async (item: InboxItem) => {
    const gateId = payloadString(item.payload, 'gate_id');
    if (!gateId) {
      setError('缺少 gate_id');
      return;
    }
    await withPending(item.id, async () => {
      await boardAgentsApi.respondPipelineGate(gateId, 'reject');
      await boardAgentsApi.markInboxRead(item.id);
    });
  };

  const handleMarkAllRead = async () => {
    const unread = items.filter((item) => !item.read_at);
    try {
      await Promise.all(
        unread.map((item) => boardAgentsApi.markInboxRead(item.id))
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const displayError = error ?? syncError?.message ?? null;

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between border-b border-border px-6 py-4">
        <div className="flex items-center gap-3">
          <TrayIcon className="size-5 text-brand" />
          <h1 className="text-lg font-semibold text-normal">Inbox</h1>
          {unreadCount > 0 && (
            <span className="rounded-full bg-brand px-2 py-0.5 text-xs text-on-brand">
              {unreadCount}
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          <label className="flex items-center gap-1.5 text-xs text-low">
            <input
              type="checkbox"
              checked={showArchived}
              onChange={(e) => setShowArchived(e.target.checked)}
            />
            显示已归档
          </label>
          {unreadCount > 0 && (
            <button
              type="button"
              className="rounded-md px-2 py-1 text-xs text-brand hover:bg-brand/10"
              onClick={() => void handleMarkAllRead()}
            >
              全部标为已读
            </button>
          )}
          <button
            type="button"
            className="rounded-md px-2 py-1 text-xs text-low hover:bg-secondary"
            onClick={() => retry()}
          >
            刷新
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-6">
        {displayError && (
          <p className="mb-4 text-sm text-destructive">{displayError}</p>
        )}
        {isLoading ? (
          <p className="text-sm text-low">加载中…</p>
        ) : items.length === 0 ? (
          <p className="text-sm text-low">暂无通知</p>
        ) : (
          <ul className="mx-auto max-w-2xl space-y-2">
            {items.map((item) => {
              const isUnread = !item.read_at;
              const isArchived = Boolean(item.archived_at);
              const busy = pending.has(item.id);
              const isMerge =
                item.type === 'merge_approval' &&
                Boolean(payloadString(item.payload, 'gate_id'));
              return (
                <li
                  key={item.id}
                  className={cn(
                    'group flex items-start gap-3 rounded-lg border border-border p-3',
                    isUnread ? 'bg-brand/5' : 'bg-secondary/40',
                    isArchived && 'opacity-60'
                  )}
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <p
                        className={cn(
                          'text-sm',
                          isUnread ? 'font-medium text-normal' : 'text-low'
                        )}
                      >
                        {item.title}
                      </p>
                      {item.type === 'merge_approval' && (
                        <span className="rounded bg-brand/15 px-1.5 py-0.5 text-[10px] text-brand">
                          合并确认
                        </span>
                      )}
                      {item.type === 'rebase_conflict' && (
                        <span className="rounded bg-destructive/15 px-1.5 py-0.5 text-[10px] text-destructive">
                          Rebase 冲突
                        </span>
                      )}
                    </div>
                    {item.body && (
                      <p className="mt-1 line-clamp-2 text-xs text-low">
                        {item.body}
                      </p>
                    )}
                    <p className="mt-1 text-[11px] text-low">
                      {new Date(item.created_at).toLocaleString()}
                    </p>
                    {isMerge && !isArchived && (
                      <div className="mt-2 flex flex-wrap gap-2">
                        <button
                          type="button"
                          disabled={busy}
                          className="rounded-md bg-brand px-2.5 py-1 text-xs text-on-brand hover:opacity-90 disabled:opacity-50"
                          onClick={() => void handleMergeApprove(item)}
                        >
                          合并
                        </button>
                        <button
                          type="button"
                          disabled={busy}
                          className="rounded-md border border-border px-2.5 py-1 text-xs text-low hover:bg-secondary disabled:opacity-50"
                          onClick={() => void handleMergeReject(item)}
                        >
                          拒绝
                        </button>
                      </div>
                    )}
                  </div>
                  <div className="flex shrink-0 items-center gap-1 opacity-0 group-hover:opacity-100">
                    {isUnread && (
                      <button
                        type="button"
                        title="标为已读"
                        disabled={busy}
                        className="rounded-md p-1.5 text-low hover:bg-brand/10 hover:text-brand disabled:opacity-50"
                        onClick={() => void handleMarkRead(item.id)}
                      >
                        <CheckIcon className="size-4" />
                      </button>
                    )}
                    {!isArchived && (
                      <button
                        type="button"
                        title="归档"
                        disabled={busy}
                        className="rounded-md p-1.5 text-low hover:bg-primary hover:text-normal disabled:opacity-50"
                        onClick={() => void handleArchive(item.id)}
                      >
                        <ArchiveIcon className="size-4" />
                      </button>
                    )}
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </div>
  );
}

export function ProjectInboxPage() {
  const { isSignedIn } = useAuth();
  const selectedOrgId = useOrganizationStore((s) => s.selectedOrgId);
  const { projectId } = useParams({ strict: false }) as { projectId: string };

  if (!isSignedIn) {
    return <LoginRequiredPrompt />;
  }
  if (!selectedOrgId || !projectId) {
    return null;
  }

  return (
    <OrgProvider organizationId={selectedOrgId}>
      <UserProvider>
        <ProjectProvider projectId={projectId}>
          <InboxInner projectId={projectId} />
        </ProjectProvider>
      </UserProvider>
    </OrgProvider>
  );
}
