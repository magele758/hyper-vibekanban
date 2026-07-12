import { useMemo, useState } from 'react';
import { PlusIcon, TrashIcon, ArrowDownIcon } from '@phosphor-icons/react';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { FolderPickerDialog } from '@/shared/dialogs/shared/FolderPickerDialog';
import { cn } from '@/shared/lib/utils';
import type {
  Agent,
  Issue,
  Squad,
  SquadPipeline,
  SquadPipelineNode,
  SquadPipelineNodeType,
  SquadTargetType,
} from 'shared/remote-types';
import {
  SquadPipelineCanvas,
  NodePalette,
  nextCanvasPosition,
  defaultLabelForType,
  type PaletteNodeType,
} from './SquadPipelineCanvas';

const TARGET_OPTIONS: {
  value: SquadTargetType;
  label: string;
  hint: string;
}[] = [
  {
    value: 'issue',
    label: 'Issue',
    hint: '以看板 Issue 为任务目标与上下文',
  },
  {
    value: 'path',
    label: '目录',
    hint: '以本地代码目录为 Agent 工作目录（运行时会创建临时 Issue）',
  },
  {
    value: 'issue_and_path',
    label: 'Issue + 目录',
    hint: 'Issue = 任务目标/上下文；目录 = Agent 本地工作目录',
  },
];

function newNodeId(): string {
  return `n_${Math.random().toString(36).slice(2, 10)}`;
}

function emptyPipeline(): SquadPipeline {
  return { nodes: [], edges: [] };
}

function nodeKind(node: SquadPipelineNode): SquadPipelineNodeType {
  return node.type ?? 'agent';
}

export type SquadEditorDraft = {
  name: string;
  leader_agent_id: string | null;
  target_type: SquadTargetType;
  issue_id: string | null;
  working_directory: string | null;
  pipeline: SquadPipeline;
};

export function squadToDraft(squad: Squad): SquadEditorDraft {
  return {
    name: squad.name,
    leader_agent_id: squad.leader_agent_id,
    target_type: squad.target_type ?? 'path',
    issue_id: squad.issue_id,
    working_directory: squad.working_directory,
    pipeline: squad.pipeline ?? emptyPipeline(),
  };
}

type Props = {
  agents: Agent[];
  issues: Issue[];
  draft: SquadEditorDraft;
  onChange: (draft: SquadEditorDraft) => void;
  onSave: () => void;
  onRun?: () => void;
  onCancel: () => void;
  busy?: boolean;
  running?: boolean;
  saveLabel?: string;
};

type EditorMode = 'canvas' | 'list';

export function SquadPipelineEditor({
  agents,
  issues,
  draft,
  onChange,
  onSave,
  onRun,
  onCancel,
  busy,
  running,
  saveLabel = '保存',
}: Props) {
  const [depSource, setDepSource] = useState<Record<string, string>>({});
  const [mode, setMode] = useState<EditorMode>('canvas');
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);

  const usesIssue =
    draft.target_type === 'issue' || draft.target_type === 'issue_and_path';
  const usesPath =
    draft.target_type === 'path' || draft.target_type === 'issue_and_path';

  const sortedIssues = useMemo(
    () =>
      [...issues].sort((a, b) =>
        (a.title || '').localeCompare(b.title || '', 'zh')
      ),
    [issues]
  );

  const selectedNode = useMemo(
    () =>
      selectedNodeId
        ? (draft.pipeline.nodes.find((n) => n.id === selectedNodeId) ?? null)
        : null,
    [draft.pipeline.nodes, selectedNodeId]
  );

  const setPipeline = (pipeline: SquadPipeline) =>
    onChange({ ...draft, pipeline });

  const updateNode = (id: string, patch: Partial<SquadPipelineNode>) => {
    setPipeline({
      ...draft.pipeline,
      nodes: draft.pipeline.nodes.map((n) =>
        n.id === id ? { ...n, ...patch } : n
      ),
    });
  };

  const addNode = (
    type: PaletteNodeType,
    position?: { x: number; y: number },
    linkFromPrev = false
  ) => {
    const id = newNodeId();
    const pos = position ?? nextCanvasPosition(draft.pipeline.nodes);
    const node: SquadPipelineNode = {
      id,
      type,
      label: defaultLabelForType(type, draft.pipeline.nodes.length),
      position: pos,
      ...(type === 'agent'
        ? { agent_id: draft.leader_agent_id ?? undefined }
        : {}),
      ...(type === 'while' ? { max_iterations: 3 } : {}),
      ...(type === 'wait' ? { wait_seconds: 5 } : {}),
    };
    const nodes = [...draft.pipeline.nodes, node];
    const edges = [...draft.pipeline.edges];
    if (linkFromPrev) {
      const prev = draft.pipeline.nodes[draft.pipeline.nodes.length - 1];
      if (prev) {
        edges.push({
          id: `e_${prev.id}_${id}`,
          source: prev.id,
          target: id,
        });
      }
    }
    setPipeline({ ...draft.pipeline, nodes, edges });
    setSelectedNodeId(id);
  };

  const addStep = () => addNode('agent', undefined, true);

  const removeStep = (id: string) => {
    setPipeline({
      ...draft.pipeline,
      nodes: draft.pipeline.nodes.filter((n) => n.id !== id),
      edges: draft.pipeline.edges.filter(
        (e) => e.source !== id && e.target !== id
      ),
    });
    if (selectedNodeId === id) setSelectedNodeId(null);
  };

  const moveStep = (index: number, dir: -1 | 1) => {
    const next = index + dir;
    if (next < 0 || next >= draft.pipeline.nodes.length) return;
    const nodes = [...draft.pipeline.nodes];
    const tmp = nodes[index];
    nodes[index] = nodes[next];
    nodes[next] = tmp;
    const edges = nodes.slice(0, -1).map((n, i) => ({
      id: `e_${n.id}_${nodes[i + 1].id}`,
      source: n.id,
      target: nodes[i + 1].id,
    }));
    setPipeline({ ...draft.pipeline, nodes, edges });
  };

  const addDependency = (targetId: string, sourceId: string) => {
    if (!sourceId || sourceId === targetId) return;
    if (
      draft.pipeline.edges.some(
        (e) => e.source === sourceId && e.target === targetId
      )
    ) {
      return;
    }
    setPipeline({
      ...draft.pipeline,
      edges: [
        ...draft.pipeline.edges,
        {
          id: `e_${sourceId}_${targetId}`,
          source: sourceId,
          target: targetId,
        },
      ],
    });
  };

  const removeEdge = (edgeId: string) => {
    setPipeline({
      ...draft.pipeline,
      edges: draft.pipeline.edges.filter((e) => e.id !== edgeId),
    });
  };

  const loop = draft.pipeline.loop_config ?? {
    max_iterations: 1,
    enabled: false,
  };

  return (
    <div className="space-y-4 rounded-lg border border-border bg-secondary p-4">
      <div className="grid gap-3 sm:grid-cols-2">
        <label className="block text-xs text-low">
          名称
          <input
            className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
            value={draft.name}
            onChange={(e) => onChange({ ...draft, name: e.target.value })}
          />
        </label>
        <label className="block text-xs text-low">
          Leader Agent（步骤未指定 Agent 时回退）
          <select
            className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
            value={draft.leader_agent_id ?? ''}
            onChange={(e) =>
              onChange({
                ...draft,
                leader_agent_id: e.target.value || null,
              })
            }
          >
            <option value="">（无）</option>
            {agents.map((a) => (
              <option key={a.id} value={a.id}>
                {a.name}
              </option>
            ))}
          </select>
        </label>
      </div>

      <div className="space-y-2 rounded-md border border-border bg-primary/40 p-3">
        <p className="text-xs font-medium text-normal">工作目标</p>
        <p className="text-xs text-low">
          Issue = 任务目标/上下文；目录 = Agent 本地工作目录。可单独或组合使用。
        </p>
        <div className="flex flex-wrap gap-2">
          {TARGET_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              type="button"
              className={cn(
                'rounded-md border px-3 py-1.5 text-xs',
                draft.target_type === opt.value
                  ? 'border-brand bg-brand/10 text-brand'
                  : 'border-border text-low hover:bg-secondary'
              )}
              onClick={() => onChange({ ...draft, target_type: opt.value })}
            >
              {opt.label}
            </button>
          ))}
        </div>
        <p className="text-xs text-low">
          {TARGET_OPTIONS.find((o) => o.value === draft.target_type)?.hint}
        </p>

        {usesIssue && (
          <label className="block text-xs text-low">
            目标 Issue
            <select
              className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
              value={draft.issue_id ?? ''}
              onChange={(e) =>
                onChange({
                  ...draft,
                  issue_id: e.target.value || null,
                })
              }
            >
              <option value="">选择 Issue…</option>
              {sortedIssues.map((iss) => (
                <option key={iss.id} value={iss.id}>
                  {iss.title || iss.id.slice(0, 8)}
                </option>
              ))}
            </select>
          </label>
        )}

        {usesPath && (
          <label className="block text-xs text-low">
            工作目录（代码库路径）
            <div className="mt-1 flex gap-2">
              <input
                className="min-w-0 flex-1 rounded-md border border-border bg-primary px-3 py-2 text-sm"
                placeholder="/path/to/repo"
                value={draft.working_directory ?? ''}
                onChange={(e) =>
                  onChange({
                    ...draft,
                    working_directory: e.target.value || null,
                  })
                }
              />
              <button
                type="button"
                className="shrink-0 rounded-md border border-border px-3 py-2 text-sm text-low hover:bg-secondary"
                onClick={() => {
                  void FolderPickerDialog.show({
                    title: '选择 Squad 工作目录',
                  }).then((path) => {
                    if (path) {
                      onChange({ ...draft, working_directory: path });
                    }
                  });
                }}
              >
                浏览…
              </button>
            </div>
          </label>
        )}
      </div>

      <div className="space-y-2">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="flex items-center gap-3">
            <p className="text-xs font-medium text-normal">
              Agent 工作流（画布编排）
            </p>
            <div className="inline-flex rounded-md border border-border p-0.5">
              {(
                [
                  { id: 'canvas', label: '画布' },
                  { id: 'list', label: '列表' },
                ] as const
              ).map((tab) => (
                <button
                  key={tab.id}
                  type="button"
                  className={cn(
                    'rounded px-2.5 py-1 text-xs',
                    mode === tab.id
                      ? 'bg-brand/10 text-brand'
                      : 'text-low hover:bg-primary'
                  )}
                  onClick={() => setMode(tab.id)}
                >
                  {tab.label}
                </button>
              ))}
            </div>
          </div>
          <button
            type="button"
            className="inline-flex items-center gap-1 text-xs text-brand hover:underline"
            onClick={addStep}
          >
            <PlusIcon className="size-3.5" />
            添加 Agent 步骤
          </button>
        </div>

        {mode === 'canvas' ? (
          <div className="space-y-2">
            <div className="space-y-1">
              <p className="text-[11px] text-low">
                并行请用 Fork → 多分支 → Join。Agent
                会排队并等待完成后再走下一步；失败可接 E(error) 出边。If 用
                T/F，While 用 B/X。
              </p>
              <NodePalette onAdd={(type) => addNode(type)} />
            </div>
            <div className="grid gap-3 lg:grid-cols-[1fr_280px]">
              <SquadPipelineCanvas
                pipeline={draft.pipeline}
                agents={agents}
                leaderAgentId={draft.leader_agent_id}
                selectedNodeId={selectedNodeId}
                onSelectNode={setSelectedNodeId}
                onChangePipeline={setPipeline}
                onAddNodeAt={(type, position) => addNode(type, position)}
              />
              <div className="rounded-md border border-border bg-primary p-3">
                {selectedNode ? (
                  <NodeDetailForm
                    node={selectedNode}
                    agents={agents}
                    onChange={(patch) => updateNode(selectedNode.id, patch)}
                    onDelete={() => removeStep(selectedNode.id)}
                  />
                ) : (
                  <p className="text-xs text-low">
                    拖入或选中节点后，在此编辑。并行：Fork 扇出 → Join
                    汇合；Agent 的 Prompt/角色会注入任务。
                  </p>
                )}
              </div>
            </div>
          </div>
        ) : draft.pipeline.nodes.length === 0 ? (
          <p className="rounded-md border border-dashed border-border px-3 py-6 text-center text-xs text-low">
            还没有步骤。切到画布：串行拖 Agent；并行用 Fork → 多个 Agent →
            Join。
          </p>
        ) : (
          <ul className="space-y-3">
            {draft.pipeline.nodes.map((node, index) => {
              const incoming = draft.pipeline.edges.filter(
                (e) => e.target === node.id
              );
              return (
                <li
                  key={node.id}
                  className="rounded-md border border-border bg-primary p-3"
                >
                  <div className="mb-2 flex items-center justify-between gap-2">
                    <span className="text-xs font-medium text-low">
                      #{index + 1} · {nodeKind(node)}
                    </span>
                    <div className="flex items-center gap-1">
                      <button
                        type="button"
                        className="rounded p-1 text-low hover:bg-secondary disabled:opacity-30"
                        disabled={index === 0}
                        onClick={() => moveStep(index, -1)}
                        title="上移"
                      >
                        ↑
                      </button>
                      <button
                        type="button"
                        className="rounded p-1 text-low hover:bg-secondary disabled:opacity-30"
                        disabled={index === draft.pipeline.nodes.length - 1}
                        onClick={() => moveStep(index, 1)}
                        title="下移"
                      >
                        <ArrowDownIcon className="size-3.5" />
                      </button>
                      <button
                        type="button"
                        className="rounded p-1 text-low hover:text-destructive"
                        onClick={() => removeStep(node.id)}
                      >
                        <TrashIcon className="size-3.5" />
                      </button>
                    </div>
                  </div>
                  <NodeDetailForm
                    node={node}
                    agents={agents}
                    onChange={(patch) => updateNode(node.id, patch)}
                    onDelete={() => removeStep(node.id)}
                    hideDelete
                  />

                  <div className="mt-2 border-t border-border pt-2">
                    <p className="mb-1 text-xs text-low">
                      依赖 / 入边（须先完成的步骤）
                    </p>
                    {incoming.length > 0 && (
                      <ul className="mb-1 space-y-0.5">
                        {incoming.map((e) => {
                          const src = draft.pipeline.nodes.find(
                            (n) => n.id === e.source
                          );
                          return (
                            <li
                              key={e.id}
                              className="flex items-center justify-between text-xs text-normal"
                            >
                              <span>
                                ← {src?.label || e.source}
                                {e.branch ? ` [${e.branch}]` : ''}
                              </span>
                              <button
                                type="button"
                                className="text-low hover:text-destructive"
                                onClick={() => removeEdge(e.id)}
                              >
                                移除
                              </button>
                            </li>
                          );
                        })}
                      </ul>
                    )}
                    <div className="flex gap-2">
                      <select
                        className="flex-1 rounded-md border border-border bg-secondary px-2 py-1 text-xs"
                        value={depSource[node.id] ?? ''}
                        onChange={(e) =>
                          setDepSource((prev) => ({
                            ...prev,
                            [node.id]: e.target.value,
                          }))
                        }
                      >
                        <option value="">添加依赖…</option>
                        {draft.pipeline.nodes
                          .filter((n) => n.id !== node.id)
                          .map((n) => (
                            <option key={n.id} value={n.id}>
                              {n.label || n.id}
                            </option>
                          ))}
                      </select>
                      <button
                        type="button"
                        className="rounded-md border border-border px-2 py-1 text-xs text-low hover:bg-secondary"
                        onClick={() => {
                          const src = depSource[node.id];
                          if (src) {
                            addDependency(node.id, src);
                            setDepSource((prev) => ({
                              ...prev,
                              [node.id]: '',
                            }));
                          }
                        }}
                      >
                        添加
                      </button>
                    </div>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </div>

      <div className="space-y-2 rounded-md border border-border bg-primary/40 p-3">
        <p className="text-xs font-medium text-normal">Loop 配置（可选）</p>
        <div className="grid gap-2 sm:grid-cols-2">
          <label className="text-xs text-low">
            最大迭代次数
            <input
              type="number"
              min={1}
              className="mt-1 w-full rounded-md border border-border bg-primary px-2 py-1.5 text-sm"
              value={loop.max_iterations ?? 1}
              onChange={(e) =>
                setPipeline({
                  ...draft.pipeline,
                  loop_config: {
                    ...loop,
                    max_iterations: Number(e.target.value) || 1,
                  },
                })
              }
            />
          </label>
          <label className="text-xs text-low">
            成功条件（文字描述）
            <input
              className="mt-1 w-full rounded-md border border-border bg-primary px-2 py-1.5 text-sm"
              placeholder="例如：所有测试通过"
              value={loop.success_condition ?? ''}
              onChange={(e) =>
                setPipeline({
                  ...draft.pipeline,
                  loop_config: {
                    ...loop,
                    success_condition: e.target.value || undefined,
                  },
                })
              }
            />
          </label>
        </div>
        <p className="text-xs text-low">
          节点级 While 优先；success_condition 会参与条件判断。定时请用
          Autopilot。并行必须用 Fork/Join（多条 Agent 出边仍是串行）。
        </p>
      </div>

      <div className="flex flex-wrap gap-2">
        <PrimaryButton disabled={busy || !draft.name.trim()} onClick={onSave}>
          {busy ? '保存中…' : saveLabel}
        </PrimaryButton>
        {onRun && (
          <button
            type="button"
            disabled={running || busy}
            className="rounded-md border border-brand px-3 py-1.5 text-sm text-brand hover:bg-brand/10 disabled:opacity-50"
            onClick={onRun}
          >
            {running ? '运行中…' : '运行一次'}
          </button>
        )}
        <button
          type="button"
          className="rounded-md px-3 py-1.5 text-sm text-low"
          onClick={onCancel}
        >
          取消
        </button>
      </div>
    </div>
  );
}

function NodeDetailForm({
  node,
  agents,
  onChange,
  onDelete,
  hideDelete,
}: {
  node: SquadPipelineNode;
  agents: Agent[];
  onChange: (patch: Partial<SquadPipelineNode>) => void;
  onDelete: () => void;
  hideDelete?: boolean;
}) {
  const kind = nodeKind(node);

  return (
    <div className="space-y-2">
      {!hideDelete && (
        <div className="mb-1 flex items-center justify-between">
          <p className="text-xs font-medium text-normal">
            {kind === 'agent' ? 'Agent 步骤' : `控制节点 · ${kind}`}
          </p>
          <button
            type="button"
            className="inline-flex items-center gap-1 text-xs text-low hover:text-destructive"
            onClick={onDelete}
          >
            <TrashIcon className="size-3.5" />
            删除
          </button>
        </div>
      )}

      <label className="block text-xs text-low">
        标签
        <input
          className="mt-1 w-full rounded-md border border-border bg-secondary px-2 py-1.5 text-sm"
          value={node.label ?? ''}
          onChange={(e) =>
            onChange({
              label: e.target.value || undefined,
            })
          }
        />
      </label>

      {kind === 'agent' && (
        <div className="grid gap-2 sm:grid-cols-2">
          <label className="text-xs text-low sm:col-span-2">
            Agent
            <select
              className="mt-1 w-full rounded-md border border-border bg-secondary px-2 py-1.5 text-sm"
              value={node.agent_id ?? ''}
              onChange={(e) =>
                onChange({
                  agent_id: e.target.value || undefined,
                })
              }
            >
              <option value="">（用 Leader）</option>
              {agents.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name}
                </option>
              ))}
            </select>
          </label>
          <label className="text-xs text-low sm:col-span-2">
            角色（可选）
            <input
              className="mt-1 w-full rounded-md border border-border bg-secondary px-2 py-1.5 text-sm"
              placeholder="reviewer / implementer…"
              value={node.role ?? ''}
              onChange={(e) =>
                onChange({
                  role: e.target.value || undefined,
                })
              }
            />
          </label>
          <label className="text-xs text-low sm:col-span-2">
            步骤说明 / Prompt
            <textarea
              className="mt-1 w-full rounded-md border border-border bg-secondary px-2 py-1.5 text-sm"
              rows={3}
              value={node.prompt ?? ''}
              onChange={(e) =>
                onChange({
                  prompt: e.target.value || undefined,
                })
              }
            />
          </label>
        </div>
      )}

      {(kind === 'if' || kind === 'while') && (
        <label className="block text-xs text-low">
          条件（true/false、status:completed、agent:关键词；空=看上一步成败）
          <input
            className="mt-1 w-full rounded-md border border-border bg-secondary px-2 py-1.5 text-sm"
            placeholder="true / status:completed / agent:pass…"
            value={node.condition ?? ''}
            onChange={(e) =>
              onChange({
                condition: e.target.value || undefined,
              })
            }
          />
        </label>
      )}

      {kind === 'fork' && (
        <p className="text-[11px] text-low">
          所有默认出边会并行执行。分支汇合请接到同一个 Join。
        </p>
      )}

      {kind === 'join' && (
        <label className="block text-xs text-low">
          需要汇合的入边数（空=全部入边）
          <input
            type="number"
            min={1}
            className="mt-1 w-full rounded-md border border-border bg-secondary px-2 py-1.5 text-sm"
            placeholder="全部"
            value={node.join_count ?? ''}
            onChange={(e) =>
              onChange({
                join_count: e.target.value
                  ? Number(e.target.value) || undefined
                  : undefined,
              })
            }
          />
        </label>
      )}

      {kind === 'while' && (
        <label className="block text-xs text-low">
          最大迭代次数
          <input
            type="number"
            min={1}
            max={20}
            className="mt-1 w-full rounded-md border border-border bg-secondary px-2 py-1.5 text-sm"
            value={node.max_iterations ?? 3}
            onChange={(e) =>
              onChange({
                max_iterations: Number(e.target.value) || 1,
              })
            }
          />
        </label>
      )}

      {kind === 'wait' && (
        <>
          <label className="block text-xs text-low">
            等待秒数（运行时最多同步睡 30s）
            <input
              type="number"
              min={0}
              className="mt-1 w-full rounded-md border border-border bg-secondary px-2 py-1.5 text-sm"
              value={node.wait_seconds ?? 0}
              onChange={(e) =>
                onChange({
                  wait_seconds: Number(e.target.value) || 0,
                })
              }
            />
          </label>
          <label className="block text-xs text-low">
            等待说明 / wait_for
            <input
              className="mt-1 w-full rounded-md border border-border bg-secondary px-2 py-1.5 text-sm"
              placeholder="例如：等测试绿"
              value={node.wait_for ?? ''}
              onChange={(e) =>
                onChange({
                  wait_for: e.target.value || undefined,
                })
              }
            />
          </label>
        </>
      )}

      {kind === 'break' && (
        <p className="text-[11px] text-low">
          运行时退出最近的 While；无 While 则忽略。
        </p>
      )}

      {kind === 'agent' && (
        <p className="text-[11px] text-low">
          运行时会入队并等待完成；失败可从 E 出边走恢复路径，否则停止该分支。
        </p>
      )}
    </div>
  );
}
