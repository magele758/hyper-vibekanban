import { useMemo, useState } from 'react';
import { PlayIcon } from '@phosphor-icons/react';
import { boardAgentsApi } from '@/shared/lib/boardAgentsApi';
import { useProjectContext } from '@/shared/hooks/useProjectContext';
import type { Squad, SquadPipelineNode } from 'shared/remote-types';

interface Props {
  issueId: string;
}

type Entry = {
  squad: Squad;
  node: SquadPipelineNode;
  label: string;
};

/**
 * Issue-panel shortcuts: "从…开始" using pipeline `entry_label`s.
 * Hidden when the project has no squads with labeled entry points (pure Issue).
 */
export function IssueWorkflowQuickRun({ issueId }: Props) {
  const { squads } = useProjectContext();
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const entries = useMemo(() => {
    const out: Entry[] = [];
    for (const squad of squads) {
      const nodes = squad.pipeline?.nodes ?? [];
      for (const node of nodes) {
        const label = node.entry_label?.trim();
        if (!label) continue;
        out.push({ squad, node, label });
      }
    }
    // Prefer closeout / full-flow names first for a stable UX.
    out.sort((a, b) => {
      const rank = (s: Squad) => {
        const n = s.name.toLowerCase();
        if (n.includes('full feature')) return 0;
        if (n.includes('closeout')) return 1;
        return 2;
      };
      return rank(a.squad) - rank(b.squad) || a.label.localeCompare(b.label);
    });
    return out;
  }, [squads]);

  if (entries.length === 0) {
    return null;
  }

  const run = async (entry: Entry) => {
    const key = `${entry.squad.id}:${entry.node.id}`;
    setBusyKey(key);
    setError(null);
    setMsg(null);
    try {
      const result = await boardAgentsApi.runSquad(entry.squad.id, {
        issue_id: issueId,
        start_from_node_id: entry.node.id,
      });
      setMsg(
        `已从「${entry.label}」启动（${entry.squad.name} · run ${result.run_id?.slice(0, 8) ?? '?'}…）`
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusyKey(null);
    }
  };

  // Deduplicate labels across squads for a compact select; keep first of each label.
  const byLabel = new Map<string, Entry>();
  for (const e of entries) {
    if (!byLabel.has(e.label)) byLabel.set(e.label, e);
  }
  const unique = [...byLabel.values()];

  return (
    <div className="border-t px-4 py-3 space-y-2">
      <div className="flex items-center justify-between gap-2">
        <span className="text-xs font-medium text-low">从步骤运行</span>
        <span className="text-[10px] text-low">不跑 pipeline 可忽略</span>
      </div>
      <div className="flex flex-wrap gap-1.5">
        {unique.map((entry) => {
          const key = `${entry.squad.id}:${entry.node.id}`;
          return (
            <button
              key={key}
              type="button"
              disabled={busyKey !== null}
              onClick={() => void run(entry)}
              className="inline-flex items-center gap-1 rounded-md border border-border bg-secondary px-2 py-1 text-xs text-normal hover:bg-brand/10 hover:border-brand/40 disabled:opacity-50"
              title={`${entry.squad.name} · ${entry.node.id}`}
            >
              <PlayIcon className="size-3" weight="fill" />
              {busyKey === key ? '启动中…' : entry.label}
            </button>
          );
        })}
      </div>
      {msg && <p className="text-xs text-brand">{msg}</p>}
      {error && <p className="text-xs text-destructive">{error}</p>}
    </div>
  );
}
