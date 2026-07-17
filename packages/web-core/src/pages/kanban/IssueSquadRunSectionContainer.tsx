import { useCallback, useEffect, useState } from 'react';
import { CheckIcon, XIcon } from '@phosphor-icons/react';
import { CollapsibleSectionHeader } from '@vibe/ui/components/CollapsibleSectionHeader';
import { boardAgentsApi } from '@/shared/lib/boardAgentsApi';
import type { SquadRun } from 'shared/remote-types';
import { cn } from '@/shared/lib/utils';

interface Props {
  issueId: string;
}

export function IssueSquadRunSectionContainer({ issueId }: Props) {
  const [runs, setRuns] = useState<SquadRun[]>([]);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    try {
      const list = await boardAgentsApi.listIssueSquadRuns(issueId);
      setRuns(list);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [issueId]);

  useEffect(() => {
    void reload();
    const t = window.setInterval(() => void reload(), 8000);
    return () => window.clearInterval(t);
  }, [reload]);

  const active = runs.filter((r) =>
    ['running', 'waiting_approval', 'queued'].includes(r.status)
  );
  if (active.length === 0 && runs.length === 0) {
    return null;
  }

  const decide = async (runId: string, decision: 'approve' | 'reject') => {
    setBusyId(runId);
    try {
      await boardAgentsApi.approveSquadRun(runId, { decision });
      await reload();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusyId(null);
    }
  };

  return (
    <CollapsibleSectionHeader
      title={`流水线 (${active.length || runs.length})`}
      persistKey="kanban-issue-squad-runs"
      defaultExpanded
    >
      <div className="space-y-2 border-t px-4 py-3">
        {error && <p className="text-xs text-destructive">{error}</p>}
        {(active.length ? active : runs.slice(0, 3)).map((run) => (
          <div
            key={run.id}
            className="rounded-md border border-border bg-secondary px-3 py-2 text-sm"
          >
            <div className="flex items-center justify-between gap-2">
              <span className="truncate text-normal">
                {run.status === 'waiting_approval'
                  ? '待你确认'
                  : run.status === 'running'
                    ? '执行中'
                    : run.status}
              </span>
              <span
                className={cn(
                  'shrink-0 text-xs',
                  run.status === 'waiting_approval' ? 'text-brand' : 'text-low'
                )}
              >
                {run.id.slice(0, 8)}…
              </span>
            </div>
            {run.approval_prompt && (
              <p className="mt-1 whitespace-pre-wrap text-xs text-low">
                {run.approval_prompt}
              </p>
            )}
            {run.status === 'waiting_approval' && (
              <div className="mt-2 flex gap-2">
                <button
                  type="button"
                  disabled={busyId === run.id}
                  className="inline-flex items-center gap-1 rounded-md border border-brand bg-brand/10 px-2 py-1 text-xs text-brand"
                  onClick={() => void decide(run.id, 'approve')}
                >
                  <CheckIcon className="size-3.5" />
                  Approve
                </button>
                <button
                  type="button"
                  disabled={busyId === run.id}
                  className="inline-flex items-center gap-1 rounded-md border border-border px-2 py-1 text-xs text-low"
                  onClick={() => void decide(run.id, 'reject')}
                >
                  <XIcon className="size-3.5" />
                  Reject
                </button>
              </div>
            )}
          </div>
        ))}
      </div>
    </CollapsibleSectionHeader>
  );
}
