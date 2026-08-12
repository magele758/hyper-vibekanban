import { useCallback, useEffect, useMemo, useState } from 'react';
import { CheckIcon, XIcon } from '@phosphor-icons/react';
import { CollapsibleSectionHeader } from '@vibe/ui/components/CollapsibleSectionHeader';
import { boardAgentsApi } from '@/shared/lib/boardAgentsApi';
import type { SquadRun } from 'shared/remote-types';
import { cn } from '@/shared/lib/utils';

interface Props {
  issueId: string;
}

const ACTIVE_STATUSES = ['running', 'waiting_approval', 'queued'];

/** Poll fast while something is in flight, slowly when idle. */
const POLL_ACTIVE_MS = 3000;
const POLL_IDLE_MS = 15000;

const STATUS_LABEL: Record<string, string> = {
  queued: '排队中',
  running: '执行中',
  waiting_approval: '待你确认',
  completed: '已完成',
  failed: '失败',
  cancelled: '已取消',
};

function formatElapsed(from: string, to: string | null): string {
  const start = new Date(from).getTime();
  const end = to ? new Date(to).getTime() : Date.now();
  const secs = Math.max(0, Math.round((end - start) / 1000));
  if (secs < 60) return `${secs}s`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m${secs % 60}s`;
  return `${Math.floor(mins / 60)}h${mins % 60}m`;
}

export function IssueSquadRunSectionContainer({ issueId }: Props) {
  const [runs, setRuns] = useState<SquadRun[]>([]);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Re-render on a timer so "执行中 4m12s" keeps ticking between polls.
  const [, setTick] = useState(0);

  const reload = useCallback(async () => {
    try {
      const list = await boardAgentsApi.listIssueSquadRuns(issueId);
      setRuns(list);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [issueId]);

  const active = useMemo(
    () => runs.filter((r) => ACTIVE_STATUSES.includes(r.status)),
    [runs]
  );
  const hasActive = active.length > 0;

  useEffect(() => {
    void reload();
    const t = window.setInterval(
      () => void reload(),
      hasActive ? POLL_ACTIVE_MS : POLL_IDLE_MS
    );
    return () => window.clearInterval(t);
  }, [reload, hasActive]);

  useEffect(() => {
    if (!hasActive) return;
    const t = window.setInterval(() => setTick((n) => n + 1), 1000);
    return () => window.clearInterval(t);
  }, [hasActive]);

  if (runs.length === 0) {
    return null;
  }

  const decide = async (
    runId: string,
    decision: 'approve' | 'reject',
    comment?: string
  ) => {
    setBusyId(runId);
    try {
      await boardAgentsApi.approveSquadRun(runId, { decision, comment });
      await reload();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusyId(null);
    }
  };

  const visible = active.length ? active : runs.slice(0, 3);

  return (
    <CollapsibleSectionHeader
      title={`流水线 (${active.length || runs.length})`}
      persistKey="kanban-issue-squad-runs"
      defaultExpanded
    >
      <div className="space-y-2 border-t px-4 py-3">
        {error && <p className="text-xs text-destructive">{error}</p>}
        {visible.map((run) => {
          const waiting = run.status === 'waiting_approval';
          const failed = run.status === 'failed';
          return (
            <div
              key={run.id}
              className={cn(
                'rounded-md border bg-secondary px-3 py-2 text-sm',
                waiting ? 'border-brand/50' : 'border-border'
              )}
            >
              <div className="flex items-center justify-between gap-2">
                <span className="truncate text-normal">
                  {STATUS_LABEL[run.status] ?? run.status}
                  {run.approval_kind && waiting && (
                    <span className="ml-1 text-xs text-brand">
                      ({run.approval_kind})
                    </span>
                  )}
                  {run.pause_node_id && waiting && (
                    <span className="ml-1 text-xs text-low">
                      @ {run.pause_node_id}
                    </span>
                  )}
                </span>
                <span
                  className={cn(
                    'shrink-0 text-xs tabular-nums',
                    waiting
                      ? 'text-brand'
                      : failed
                        ? 'text-destructive'
                        : 'text-low'
                  )}
                >
                  {formatElapsed(run.started_at, run.completed_at)}
                </span>
              </div>

              {run.approval_prompt && waiting && (
                <p className="mt-1 whitespace-pre-wrap text-xs text-low">
                  {run.approval_prompt}
                </p>
              )}

              {/* Surfacing this is the difference between "it silently
                  stopped" and "here's why it stopped". */}
              {failed && run.error_message && (
                <p className="mt-1 whitespace-pre-wrap break-words text-xs text-destructive">
                  {run.error_message}
                </p>
              )}

              {waiting && (
                <div className="mt-2 flex gap-2">
                  <button
                    type="button"
                    disabled={busyId === run.id}
                    className="inline-flex items-center gap-1 rounded-md border border-brand bg-brand/10 px-2 py-1 text-xs text-brand disabled:opacity-50"
                    onClick={() => void decide(run.id, 'approve')}
                  >
                    <CheckIcon className="size-3.5" />
                    {busyId === run.id ? '提交中…' : 'Approve'}
                  </button>
                  <button
                    type="button"
                    disabled={busyId === run.id}
                    className="inline-flex items-center gap-1 rounded-md border border-border px-2 py-1 text-xs text-low disabled:opacity-50"
                    onClick={() => {
                      const comment =
                        window.prompt('驳回原因（可留空）：') ?? undefined;
                      void decide(run.id, 'reject', comment);
                    }}
                  >
                    <XIcon className="size-3.5" />
                    Reject
                  </button>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </CollapsibleSectionHeader>
  );
}
