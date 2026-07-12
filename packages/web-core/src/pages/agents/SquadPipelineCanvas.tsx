import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { DragEvent } from 'react';
import {
  Background,
  BackgroundVariant,
  Controls,
  Handle,
  MarkerType,
  MiniMap,
  Position,
  ReactFlow,
  applyEdgeChanges,
  applyNodeChanges,
  useReactFlow,
  ReactFlowProvider,
  type Connection,
  type Edge,
  type EdgeChange,
  type Node,
  type NodeChange,
  type NodeProps,
  type OnEdgesChange,
  type OnNodesChange,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import {
  RobotIcon,
  GitBranchIcon,
  ArrowsClockwiseIcon,
  ProhibitIcon,
  TimerIcon,
  TreeStructureIcon,
  ArrowsInIcon,
} from '@phosphor-icons/react';
import { cn } from '@/shared/lib/utils';
import type {
  Agent,
  SquadPipeline,
  SquadPipelineEdgeBranch,
  SquadPipelineNode,
  SquadPipelineNodeType,
} from 'shared/remote-types';

export type PaletteNodeType = SquadPipelineNodeType;

export const PALETTE_ITEMS: {
  type: PaletteNodeType;
  label: string;
  hint: string;
}[] = [
  { type: 'agent', label: 'Agent 步骤', hint: '执行并等待 Agent 任务完成' },
  { type: 'fork', label: 'Fork', hint: '并行扇出：所有默认出边同时执行' },
  { type: 'join', label: 'Join', hint: '汇合屏障：等入边分支都完成后再继续' },
  { type: 'if', label: 'If / Else', hint: '条件分支 true/false' },
  { type: 'while', label: 'While', hint: '循环 body / exit' },
  { type: 'break', label: 'Break', hint: '跳出最近 While' },
  { type: 'wait', label: 'Wait', hint: '等待时长或条件' },
];

const DND_MIME = 'application/vk-squad-node';

export type PipelineFlowNodeData = {
  nodeType: SquadPipelineNodeType;
  label: string;
  agentName: string;
  role?: string;
  promptSnippet: string;
  condition?: string;
  detail?: string;
  selected: boolean;
};

type PipelineFlowNode = Node<PipelineFlowNodeData, 'pipelineStep'>;

const NODE_W = 200;
const NODE_H = 88;
const COL_GAP = 56;
const ROW_GAP = 28;

function defaultPosition(index: number): { x: number; y: number } {
  const col = index % 3;
  const row = Math.floor(index / 3);
  return {
    x: col * (NODE_W + COL_GAP) + 24,
    y: row * (NODE_H + ROW_GAP) + 24,
  };
}

function promptSnippet(prompt?: string): string {
  if (!prompt?.trim()) return '';
  const oneLine = prompt.replace(/\s+/g, ' ').trim();
  return oneLine.length > 48 ? `${oneLine.slice(0, 48)}…` : oneLine;
}

function agentLabel(
  agents: Agent[],
  agentId: string | undefined,
  leaderId: string | null
): string {
  const id = agentId || leaderId || undefined;
  if (!id) return '未指定 Agent';
  const agent = agents.find((a) => a.id === id);
  const name = agent?.name ?? id.slice(0, 8);
  return agentId ? name : `${name}（Leader）`;
}

function nodeKind(node: SquadPipelineNode): SquadPipelineNodeType {
  return node.type ?? 'agent';
}

function kindChrome(kind: SquadPipelineNodeType): {
  bar: string;
  icon: typeof RobotIcon;
  badge: string;
} {
  switch (kind) {
    case 'agent':
      return { bar: 'border-l-brand', icon: RobotIcon, badge: 'Agent' };
    case 'fork':
      return {
        bar: 'border-l-emerald-500',
        icon: TreeStructureIcon,
        badge: 'Fork',
      };
    case 'join':
      return {
        bar: 'border-l-teal-500',
        icon: ArrowsInIcon,
        badge: 'Join',
      };
    case 'if':
      return {
        bar: 'border-l-amber-500',
        icon: GitBranchIcon,
        badge: 'If',
      };
    case 'while':
      return {
        bar: 'border-l-sky-500',
        icon: ArrowsClockwiseIcon,
        badge: 'While',
      };
    case 'break':
      return {
        bar: 'border-l-rose-500',
        icon: ProhibitIcon,
        badge: 'Break',
      };
    case 'wait':
      return {
        bar: 'border-l-violet-500',
        icon: TimerIcon,
        badge: 'Wait',
      };
    default: {
      const _exhaustive: never = kind;
      return _exhaustive;
    }
  }
}

function controlDetail(node: SquadPipelineNode): string | undefined {
  const kind = nodeKind(node);
  switch (kind) {
    case 'agent':
      return undefined;
    case 'fork':
      return '并行扇出默认出边';
    case 'join':
      return node.join_count != null && node.join_count > 0
        ? `等 ${node.join_count} 条入边`
        : '等全部入边汇合';
    case 'if':
      return node.condition?.trim()
        ? `if ${node.condition.trim()}`
        : '条件未设置（看上一步结果）';
    case 'while':
      return [
        node.condition?.trim() || '条件看上一步',
        `max ${node.max_iterations ?? 3}`,
      ].join(' · ');
    case 'wait': {
      const parts: string[] = [];
      if (node.wait_seconds != null && node.wait_seconds > 0) {
        parts.push(`${node.wait_seconds}s`);
      }
      if (node.wait_for?.trim()) parts.push(node.wait_for.trim());
      return parts.length ? parts.join(' · ') : '无等待配置';
    }
    case 'break':
      return '退出最近 While';
    default: {
      const _exhaustive: never = kind;
      return _exhaustive;
    }
  }
}

export function pipelineToFlowNodes(
  pipeline: SquadPipeline,
  agents: Agent[],
  leaderAgentId: string | null,
  selectedId: string | null
): PipelineFlowNode[] {
  return pipeline.nodes.map((node, index) => {
    const kind = nodeKind(node);
    const pos = node.position
      ? { x: node.position.x, y: node.position.y }
      : defaultPosition(index);
    return {
      id: node.id,
      type: 'pipelineStep' as const,
      position: pos,
      selected: selectedId === node.id,
      data: {
        nodeType: kind,
        label:
          node.label?.trim() ||
          (kind === 'agent' ? `步骤 ${index + 1}` : kindChrome(kind).badge),
        agentName:
          kind === 'agent'
            ? agentLabel(agents, node.agent_id, leaderAgentId)
            : kindChrome(kind).badge,
        role: node.role?.trim() || undefined,
        promptSnippet: promptSnippet(node.prompt),
        condition: node.condition?.trim() || undefined,
        detail: controlDetail(node),
        selected: selectedId === node.id,
      },
    };
  });
}

function branchLabel(
  branch?: SquadPipelineEdgeBranch | null
): string | undefined {
  if (!branch || branch === 'default') return undefined;
  return branch;
}

export function pipelineToFlowEdges(pipeline: SquadPipeline): Edge[] {
  return pipeline.edges.map((e) => {
    const label = branchLabel(e.branch);
    return {
      id: e.id,
      source: e.source,
      target: e.target,
      sourceHandle: e.branch && e.branch !== 'default' ? e.branch : undefined,
      label,
      labelStyle: { fontSize: 10, fill: 'var(--color-low, #888)' },
      markerEnd: { type: MarkerType.ArrowClosed, width: 16, height: 16 },
      style: { strokeWidth: 1.5 },
    };
  });
}

function PipelineStepNode({ data }: NodeProps<PipelineFlowNode>) {
  const chrome = kindChrome(data.nodeType);
  const Icon = chrome.icon;
  const isIf = data.nodeType === 'if';
  const isWhile = data.nodeType === 'while';
  const isAgent = data.nodeType === 'agent';
  const multiOut = isIf || isWhile;
  // Agent can optionally expose an error handle for recovery paths.
  const agentErrorOut = isAgent;

  return (
    <div
      className={cn(
        'relative w-[200px] rounded-md border border-l-4 bg-primary px-2.5 py-2 shadow-sm',
        chrome.bar,
        data.selected ? 'border-brand ring-1 ring-brand/40' : 'border-border'
      )}
    >
      <Handle
        type="target"
        position={Position.Left}
        className="!size-2.5 !border-border !bg-secondary"
      />
      <div className="flex items-center gap-1.5">
        <Icon className="size-3.5 shrink-0 text-low" />
        <p className="truncate text-xs font-medium text-normal">{data.label}</p>
      </div>
      {data.nodeType === 'agent' ? (
        <>
          <p className="mt-0.5 truncate text-[11px] text-low">
            {data.agentName}
          </p>
          {data.role ? (
            <p className="mt-0.5 truncate text-[10px] text-low">{data.role}</p>
          ) : null}
          {data.promptSnippet ? (
            <p className="mt-1 line-clamp-2 text-[10px] leading-snug text-low">
              {data.promptSnippet}
            </p>
          ) : null}
        </>
      ) : (
        <p className="mt-1 line-clamp-2 text-[10px] leading-snug text-low">
          {data.detail}
        </p>
      )}
      {multiOut ? (
        <>
          <Handle
            type="source"
            id={isIf ? 'true' : 'body'}
            position={Position.Right}
            style={{ top: '35%' }}
            className="!size-2.5 !border-border !bg-emerald-500"
          />
          <Handle
            type="source"
            id={isIf ? 'false' : 'exit'}
            position={Position.Right}
            style={{ top: '70%' }}
            className="!size-2.5 !border-border !bg-rose-500"
          />
          <div className="pointer-events-none absolute right-1 top-[28%] text-[9px] text-low">
            {isIf ? 'T' : 'B'}
          </div>
          <div className="pointer-events-none absolute right-1 top-[63%] text-[9px] text-low">
            {isIf ? 'F' : 'X'}
          </div>
        </>
      ) : agentErrorOut ? (
        <>
          <Handle
            type="source"
            id="default"
            position={Position.Right}
            style={{ top: '35%' }}
            className="!size-2.5 !border-border !bg-brand"
          />
          <Handle
            type="source"
            id="error"
            position={Position.Right}
            style={{ top: '70%' }}
            className="!size-2.5 !border-border !bg-rose-500"
          />
          <div className="pointer-events-none absolute right-1 top-[28%] text-[9px] text-low">
            →
          </div>
          <div className="pointer-events-none absolute right-1 top-[63%] text-[9px] text-low">
            E
          </div>
        </>
      ) : (
        <Handle
          type="source"
          position={Position.Right}
          className="!size-2.5 !border-border !bg-brand"
        />
      )}
    </div>
  );
}

const nodeTypes = { pipelineStep: PipelineStepNode };

type Props = {
  pipeline: SquadPipeline;
  agents: Agent[];
  leaderAgentId: string | null;
  selectedNodeId: string | null;
  onSelectNode: (id: string | null) => void;
  onChangePipeline: (pipeline: SquadPipeline) => void;
  onAddNodeAt?: (
    type: PaletteNodeType,
    position: { x: number; y: number }
  ) => void;
};

function SquadPipelineCanvasInner({
  pipeline,
  agents,
  leaderAgentId,
  selectedNodeId,
  onSelectNode,
  onChangePipeline,
  onAddNodeAt,
}: Props) {
  const pipelineRef = useRef(pipeline);
  pipelineRef.current = pipeline;
  const { screenToFlowPosition } = useReactFlow();

  const flowNodes = useMemo(
    () => pipelineToFlowNodes(pipeline, agents, leaderAgentId, selectedNodeId),
    [pipeline, agents, leaderAgentId, selectedNodeId]
  );

  const flowEdges = useMemo(() => pipelineToFlowEdges(pipeline), [pipeline]);

  const [nodes, setNodes] = useState(flowNodes);
  const [edges, setEdges] = useState(flowEdges);

  const nodeSig = useMemo(() => nodeSignature(flowNodes), [flowNodes]);
  const edgeSig = useMemo(() => edgeSignature(flowEdges), [flowEdges]);
  const prevNodeSig = useRef(nodeSig);
  const prevEdgeSig = useRef(edgeSig);

  useEffect(() => {
    if (prevNodeSig.current !== nodeSig) {
      prevNodeSig.current = nodeSig;
      setNodes(flowNodes);
    }
  }, [nodeSig, flowNodes]);

  useEffect(() => {
    if (prevEdgeSig.current !== edgeSig) {
      prevEdgeSig.current = edgeSig;
      setEdges(flowEdges);
    }
  }, [edgeSig, flowEdges]);

  const persistPositions = useCallback(
    (nextNodes: PipelineFlowNode[]) => {
      const draft = pipelineRef.current;
      let changed = false;
      const mapped = draft.nodes.map((n) => {
        const fn = nextNodes.find((x) => x.id === n.id);
        if (!fn) return n;
        const x = Math.round(fn.position.x);
        const y = Math.round(fn.position.y);
        if (n.position?.x === x && n.position?.y === y) return n;
        changed = true;
        return { ...n, position: { x, y } };
      });
      if (changed) {
        onChangePipeline({ ...draft, nodes: mapped });
      }
    },
    [onChangePipeline]
  );

  const onNodesChange: OnNodesChange<PipelineFlowNode> = useCallback(
    (changes: NodeChange<PipelineFlowNode>[]) => {
      setNodes((prev) => {
        const next = applyNodeChanges(changes, prev);
        const finished = changes.some(
          (c) =>
            c.type === 'position' && 'dragging' in c && c.dragging === false
        );
        if (finished) {
          persistPositions(next);
        }
        return next;
      });
    },
    [persistPositions]
  );

  const onEdgesChange: OnEdgesChange = useCallback(
    (changes: EdgeChange[]) => {
      setEdges((prev) => applyEdgeChanges(changes, prev));
      const removals = changes.filter((c) => c.type === 'remove');
      if (removals.length === 0) return;
      const removeIds = new Set(
        removals.map((c) => (c.type === 'remove' ? c.id : ''))
      );
      const draft = pipelineRef.current;
      onChangePipeline({
        ...draft,
        edges: draft.edges.filter((e) => !removeIds.has(e.id)),
      });
    },
    [onChangePipeline]
  );

  const onConnect = useCallback(
    (connection: Connection) => {
      const source = connection.source;
      const target = connection.target;
      if (!source || !target || source === target) return;
      const draft = pipelineRef.current;
      const handle = connection.sourceHandle;
      const branch: SquadPipelineEdgeBranch | undefined =
        handle === 'true' ||
        handle === 'false' ||
        handle === 'body' ||
        handle === 'exit' ||
        handle === 'error'
          ? handle
          : undefined;
      if (
        draft.edges.some(
          (e) =>
            e.source === source &&
            e.target === target &&
            (e.branch ?? undefined) === branch
        )
      ) {
        return;
      }
      const branchSuffix = branch ? `_${branch}` : '';
      onChangePipeline({
        ...draft,
        edges: [
          ...draft.edges,
          {
            id: `e_${source}_${target}${branchSuffix}`,
            source,
            target,
            branch,
          },
        ],
      });
    },
    [onChangePipeline]
  );

  const onNodesDelete = useCallback(
    (deleted: PipelineFlowNode[]) => {
      const ids = new Set(deleted.map((n) => n.id));
      const draft = pipelineRef.current;
      onChangePipeline({
        ...draft,
        nodes: draft.nodes.filter((n) => !ids.has(n.id)),
        edges: draft.edges.filter(
          (e) => !ids.has(e.source) && !ids.has(e.target)
        ),
      });
      if (selectedNodeId && ids.has(selectedNodeId)) {
        onSelectNode(null);
      }
    },
    [onChangePipeline, onSelectNode, selectedNodeId]
  );

  const onEdgesDelete = useCallback(
    (deleted: Edge[]) => {
      const ids = new Set(deleted.map((e) => e.id));
      const draft = pipelineRef.current;
      onChangePipeline({
        ...draft,
        edges: draft.edges.filter((e) => !ids.has(e.id)),
      });
    },
    [onChangePipeline]
  );

  const onDragOver = useCallback((event: DragEvent) => {
    event.preventDefault();
    event.dataTransfer.dropEffect = 'move';
  }, []);

  const onDrop = useCallback(
    (event: DragEvent) => {
      event.preventDefault();
      const raw = event.dataTransfer.getData(DND_MIME);
      if (!raw || !onAddNodeAt) return;
      const type = raw as PaletteNodeType;
      if (!PALETTE_ITEMS.some((p) => p.type === type)) return;
      const position = screenToFlowPosition({
        x: event.clientX,
        y: event.clientY,
      });
      onAddNodeAt(type, {
        x: Math.round(position.x),
        y: Math.round(position.y),
      });
    },
    [onAddNodeAt, screenToFlowPosition]
  );

  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      nodeTypes={nodeTypes}
      onNodesChange={onNodesChange}
      onEdgesChange={onEdgesChange}
      onConnect={onConnect}
      onNodesDelete={onNodesDelete}
      onEdgesDelete={onEdgesDelete}
      onNodeClick={(_, node) => onSelectNode(node.id)}
      onPaneClick={() => onSelectNode(null)}
      onDragOver={onDragOver}
      onDrop={onDrop}
      fitView
      fitViewOptions={{ padding: 0.2 }}
      deleteKeyCode={['Backspace', 'Delete']}
      proOptions={{ hideAttribution: true }}
      defaultEdgeOptions={{
        markerEnd: { type: MarkerType.ArrowClosed, width: 16, height: 16 },
      }}
    >
      <Background
        variant={BackgroundVariant.Dots}
        gap={16}
        size={1}
        className="!bg-primary"
      />
      <Controls showInteractive={false} />
      <MiniMap
        pannable
        zoomable
        className="!bg-secondary !border-border"
        maskColor="rgb(0 0 0 / 0.08)"
      />
    </ReactFlow>
  );
}

export function SquadPipelineCanvas(props: Props) {
  return (
    <div className="h-[420px] w-full overflow-hidden rounded-md border border-border bg-primary">
      <ReactFlowProvider>
        <SquadPipelineCanvasInner {...props} />
      </ReactFlowProvider>
    </div>
  );
}

export function NodePalette({
  onAdd,
}: {
  onAdd?: (type: PaletteNodeType) => void;
}) {
  return (
    <div className="flex flex-wrap gap-1.5">
      {PALETTE_ITEMS.map((item) => {
        const chrome = kindChrome(item.type);
        const Icon = chrome.icon;
        return (
          <div
            key={item.type}
            draggable
            title={item.hint}
            onDragStart={(e) => {
              e.dataTransfer.setData(DND_MIME, item.type);
              e.dataTransfer.effectAllowed = 'move';
            }}
            onClick={() => onAdd?.(item.type)}
            className={cn(
              'inline-flex cursor-grab items-center gap-1 rounded-md border border-l-4 border-border bg-primary px-2 py-1 text-[11px] text-normal active:cursor-grabbing hover:bg-secondary',
              chrome.bar
            )}
          >
            <Icon className="size-3.5 text-low" />
            {item.label}
          </div>
        );
      })}
    </div>
  );
}

/** Next default position for a newly added step. */
export function nextCanvasPosition(nodes: SquadPipelineNode[]): {
  x: number;
  y: number;
} {
  if (nodes.length === 0) return defaultPosition(0);
  const withPos = nodes.filter((n) => n.position);
  if (withPos.length === 0) return defaultPosition(nodes.length);
  const maxX = Math.max(...withPos.map((n) => n.position!.x));
  const avgY = Math.round(
    withPos.reduce((s, n) => s + n.position!.y, 0) / withPos.length
  );
  return { x: maxX + NODE_W + COL_GAP, y: avgY };
}

export function defaultLabelForType(
  type: PaletteNodeType,
  index: number
): string {
  switch (type) {
    case 'agent':
      return `步骤 ${index + 1}`;
    case 'fork':
      return 'Fork';
    case 'join':
      return 'Join';
    case 'if':
      return 'If / Else';
    case 'while':
      return 'While';
    case 'break':
      return 'Break';
    case 'wait':
      return 'Wait';
    default: {
      const _exhaustive: never = type;
      return _exhaustive;
    }
  }
}

function nodeSignature(nodes: PipelineFlowNode[]): string {
  return nodes
    .map(
      (n) =>
        `${n.id}:${n.position.x},${n.position.y}:${n.data.nodeType}:${n.data.label}:${n.data.agentName}:${n.data.role ?? ''}:${n.data.promptSnippet}:${n.data.detail ?? ''}:${n.selected ? 1 : 0}`
    )
    .join('|');
}

function edgeSignature(edges: Edge[]): string {
  return edges
    .map(
      (e) =>
        `${e.id}:${e.source}->${e.target}:${e.sourceHandle ?? ''}:${String(e.label ?? '')}`
    )
    .join('|');
}
