import { useMemo, useState } from 'react';
import { useParams } from '@tanstack/react-router';
import {
  TrayIcon,
  CheckIcon,
  ArchiveIcon,
  CheckCircle,
  XCircle,
  Warning,
  Info,
} from '@phosphor-icons/react';
import { OrgProvider } from '@/shared/providers/remote/OrgProvider';
import { ProjectProvider } from '@/shared/providers/remote/ProjectProvider';
import { UserProvider } from '@/shared/providers/remote/UserProvider';
import { useOrganizationStore } from '@/shared/stores/useOrganizationStore';
import { useAuth } from '@/shared/hooks/auth/useAuth';
import { useUserContext } from '@/shared/hooks/useUserContext';
import { LoginRequiredPrompt } from '@/shared/dialogs/shared/LoginRequiredPrompt';
import { boardAgentsApi } from '@/shared/lib/boardAgentsApi';
import { cn } from '@/shared/lib/utils';
import type { InboxItem } from 'shared/remote-types';

type SeverityLevel = 'normal' | 'success' | 'needs-approval' | 'error';

function classifySeverity(item: InboxItem): SeverityLevel {
  if (item.type === 'workflow_approval') {
    return 'needs-approval';
  }
  if (item.type === 'agent_task') {
    const payload = item.payload as { status?: string } | null;
    const status = payload?.status;
    if (status === 'completed') return 'success';
    if (status === 'failed') return 'error';
  }
  return 'normal';
}

const severityConfig: Record<
  SeverityLevel,
  { border: string; Icon: typeof Info; iconColor: string }
> = {
  normal: {
    border: 'border-l-4 border-l-border',
    Icon: Info,
    iconColor: 'text-low',
  },
  success: {
    border: 'border-l-4 border-l-green-500',
    Icon: CheckCircle,
    iconColor: 'text-green-600',
  },
  'needs-approval': {
    border: 'border-l-4 border-l-yellow-500',
    Icon: Warning,
    iconColor: 'text-yellow-600',
  },
  error: {
    border: 'border-l-4 border-l-red-500',
    Icon: XCircle,
    iconColor: 'text-red-600',
  },
};

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

  const handleMarkRead = async (id: string) => {
    setPending((prev) => new Set(prev).add(id));
    try {
      await boardAgentsApi.markInboxRead(id);
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

  const handleArchive = async (id: string) => {
    setPending((prev) => new Set(prev).add(id));
    try {
      await boardAgentsApi.archiveInboxItem(id);
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
              const severity = classifySeverity(item);
              const { border, Icon, iconColor } = severityConfig[severity];
              return (
                <li
                  key={item.id}
                  className={cn(
                    'group flex items-start gap-3 rounded-lg border border-border p-3',
                    border,
                    isUnread ? 'bg-brand/5' : 'bg-secondary/40',
                    isArchived && 'opacity-60'
                  )}
                >
                  <Icon className={cn('mt-0.5 size-4 shrink-0', iconColor)} />
                  <div className="min-w-0 flex-1">
                    <p
                      className={cn(
                        'text-sm',
                        isUnread ? 'font-medium text-normal' : 'text-low'
                      )}
                    >
                      {item.title}
                    </p>
                    {item.body && (
                      <p className="mt-1 line-clamp-2 text-xs text-low">
                        {item.body}
                      </p>
                    )}
                    {item.type === 'workflow_approval' &&
                      (() => {
                        const payload = item.payload as {
                          squad_run_id?: string;
                        } | null;
                        const runId = payload?.squad_run_id;
                        if (!runId) return null;
                        return (
                          <div className="mt-2 flex gap-2">
                            <button
                              type="button"
                              disabled={busy}
                              className="rounded-md border border-brand bg-brand/10 px-2 py-1 text-xs text-brand disabled:opacity-50"
                              onClick={() => {
                                void (async () => {
                                  setPending((prev) =>
                                    new Set(prev).add(item.id)
                                  );
                                  try {
                                    await boardAgentsApi.approveSquadRun(
                                      runId,
                                      { decision: 'approve' }
                                    );
                                    await boardAgentsApi.markInboxRead(item.id);
                                  } catch (e) {
                                    setError(
                                      e instanceof Error ? e.message : String(e)
                                    );
                                  } finally {
                                    setPending((prev) => {
                                      const next = new Set(prev);
                                      next.delete(item.id);
                                      return next;
                                    });
                                  }
                                })();
                              }}
                            >
                              Approve
                            </button>
                            <button
                              type="button"
                              disabled={busy}
                              className="rounded-md border border-border px-2 py-1 text-xs text-low disabled:opacity-50"
                              onClick={() => {
                                void (async () => {
                                  setPending((prev) =>
                                    new Set(prev).add(item.id)
                                  );
                                  try {
                                    await boardAgentsApi.approveSquadRun(
                                      runId,
                                      { decision: 'reject' }
                                    );
                                    await boardAgentsApi.markInboxRead(item.id);
                                  } catch (e) {
                                    setError(
                                      e instanceof Error ? e.message : String(e)
                                    );
                                  } finally {
                                    setPending((prev) => {
                                      const next = new Set(prev);
                                      next.delete(item.id);
                                      return next;
                                    });
                                  }
                                })();
                              }}
                            >
                              Reject
                            </button>
                          </div>
                        );
                      })()}
                    <p className="mt-1 text-[11px] text-low">
                      {new Date(item.created_at).toLocaleString()}
                    </p>
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
