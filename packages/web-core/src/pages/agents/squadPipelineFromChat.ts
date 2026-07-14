/**
 * Conversational Squad pipeline generation.
 * Deterministic Chinese/English NL templates + optional LLM (agent sidecar).
 * Validates against SquadPipeline types; fuzzy-matches agent names.
 */
import type {
  Agent,
  Issue,
  SquadPipeline,
  SquadPipelineEdge,
  SquadPipelineEdgeBranch,
  SquadPipelineNode,
  SquadPipelineNodeType,
  SquadTargetType,
} from 'shared/remote-types';
import { boardAgentsApi } from '@/shared/lib/boardAgentsApi';
import { getAuthRuntime } from '@/shared/lib/auth/runtime';
import type { SquadEditorDraft } from './SquadPipelineEditor';

const NODE_W = 200;
const NODE_H = 88;
const COL_GAP = 56;
const ROW_GAP = 36;

const NODE_TYPES: SquadPipelineNodeType[] = [
  'agent',
  'if',
  'while',
  'break',
  'wait',
  'fork',
  'join',
  'rebase',
  'human_gate',
];

const BRANCHES: SquadPipelineEdgeBranch[] = [
  'default',
  'true',
  'false',
  'body',
  'exit',
  'error',
];

export type SquadChatGeneration = {
  summary: string;
  draft: SquadEditorDraft;
  warnings: string[];
  source: 'template' | 'llm' | 'patch';
  /** Set when LLM was attempted but fell back to template. */
  llmError?: string;
};

export type SquadChatGenStatus = 'checking' | 'llm' | 'template';

type AgentRef = { nameHint: string; role?: string; prompt?: string };

type StepSpec =
  | { kind: 'agent'; ref: AgentRef }
  | { kind: 'wait'; seconds?: number; waitFor?: string; label?: string }
  | { kind: 'if'; condition: string; then: StepSpec[]; else?: StepSpec[] }
  | {
      kind: 'while';
      condition: string;
      body: StepSpec[];
      maxIterations?: number;
    }
  | { kind: 'break' }
  | { kind: 'parallel'; branches: StepSpec[][] }
  | { kind: 'seq'; steps: StepSpec[] };

function newId(prefix: string): string {
  return `${prefix}_${Math.random().toString(36).slice(2, 10)}`;
}

function normalize(s: string): string {
  return s
    .toLowerCase()
    .replace(/[\s_\-·•]+/g, '')
    .replace(/[，。！？、；：""''（）()【】\[\]{}]/g, '');
}

/** Fuzzy match agent by display name (substring / shared chars). */
export function matchAgentByName(
  agents: Agent[],
  hint: string
): Agent | undefined {
  const h = hint.trim();
  if (!h) return undefined;
  const exact = agents.find((a) => a.name === h);
  if (exact) return exact;
  const nh = normalize(h);
  if (!nh) return undefined;
  const byNorm = agents.find((a) => normalize(a.name) === nh);
  if (byNorm) return byNorm;
  const contains = agents.filter(
    (a) => normalize(a.name).includes(nh) || nh.includes(normalize(a.name))
  );
  if (contains.length === 1) return contains[0];
  if (contains.length > 1) {
    contains.sort(
      (a, b) =>
        Math.abs(normalize(a.name).length - nh.length) -
        Math.abs(normalize(b.name).length - nh.length)
    );
    return contains[0];
  }
  // Shared character ratio for short Chinese names
  let best: Agent | undefined;
  let bestScore = 0;
  for (const a of agents) {
    const an = normalize(a.name);
    if (!an) continue;
    let shared = 0;
    const set = new Set(an);
    for (const ch of nh) if (set.has(ch)) shared += 1;
    const score = shared / Math.max(nh.length, an.length);
    if (score > bestScore && score >= 0.5) {
      bestScore = score;
      best = a;
    }
  }
  return best;
}

function layoutSequential(
  nodes: SquadPipelineNode[],
  startCol = 0,
  row = 0
): void {
  nodes.forEach((n, i) => {
    if (n.position) return;
    n.position = {
      x: (startCol + i) * (NODE_W + COL_GAP) + 24,
      y: row * (NODE_H + ROW_GAP) + 24,
    };
  });
}

function assignMissingPositions(pipeline: SquadPipeline): void {
  const byId = new Map(pipeline.nodes.map((n) => [n.id, n]));
  const outs = new Map<string, string[]>();
  const ins = new Map<string, number>();
  for (const n of pipeline.nodes) {
    outs.set(n.id, []);
    ins.set(n.id, 0);
  }
  for (const e of pipeline.edges) {
    outs.get(e.source)?.push(e.target);
    ins.set(e.target, (ins.get(e.target) ?? 0) + 1);
  }
  const roots = pipeline.nodes.filter((n) => (ins.get(n.id) ?? 0) === 0);
  const visited = new Set<string>();
  let col = 0;
  const queue = roots.map((n) => ({ id: n.id, col: 0, row: 0 }));
  const rowAtCol = new Map<number, number>();

  while (queue.length) {
    const cur = queue.shift()!;
    if (visited.has(cur.id)) continue;
    visited.add(cur.id);
    const node = byId.get(cur.id);
    if (!node) continue;
    const row = rowAtCol.get(cur.col) ?? 0;
    rowAtCol.set(cur.col, row + 1);
    if (!node.position) {
      node.position = {
        x: cur.col * (NODE_W + COL_GAP) + 24,
        y: row * (NODE_H + ROW_GAP) + 24,
      };
    }
    col = Math.max(col, cur.col);
    for (const t of outs.get(cur.id) ?? []) {
      queue.push({ id: t, col: cur.col + 1, row: 0 });
    }
  }
  // orphans
  pipeline.nodes.forEach((n, i) => {
    if (!n.position) {
      n.position = {
        x: (col + 1) * (NODE_W + COL_GAP) + 24,
        y: i * (NODE_H + ROW_GAP) + 24,
      };
    }
  });
}

type BuildCtx = {
  agents: Agent[];
  warnings: string[];
  nodes: SquadPipelineNode[];
  edges: SquadPipelineEdge[];
};

function resolveAgent(
  ctx: BuildCtx,
  ref: AgentRef
): { agent_id?: string; label: string; role?: string; prompt?: string } {
  const matched = matchAgentByName(ctx.agents, ref.nameHint);
  if (!matched && ref.nameHint) {
    ctx.warnings.push(`未匹配到 Agent「${ref.nameHint}」，已留空待指派`);
  }
  return {
    agent_id: matched?.id,
    label: ref.nameHint || matched?.name || 'Agent 步骤',
    role: ref.role,
    prompt: ref.prompt,
  };
}

function link(
  ctx: BuildCtx,
  source: string,
  target: string,
  branch?: SquadPipelineEdgeBranch
): void {
  ctx.edges.push({
    id: newId('e'),
    source,
    target,
    ...(branch && branch !== 'default' ? { branch } : {}),
  });
}

/** Build nodes/edges from StepSpec; returns entry ids and exit ids. */
function emit(
  ctx: BuildCtx,
  spec: StepSpec
): { entries: string[]; exits: string[] } {
  switch (spec.kind) {
    case 'seq': {
      if (spec.steps.length === 0) return { entries: [], exits: [] };
      let entries: string[] = [];
      let exits: string[] = [];
      for (let i = 0; i < spec.steps.length; i++) {
        const part = emit(ctx, spec.steps[i]!);
        if (i === 0) entries = part.entries;
        else {
          for (const prev of exits) {
            for (const next of part.entries) link(ctx, prev, next);
          }
        }
        exits = part.exits;
      }
      return { entries, exits };
    }
    case 'agent': {
      const resolved = resolveAgent(ctx, spec.ref);
      const id = newId('n');
      ctx.nodes.push({
        id,
        type: 'agent',
        agent_id: resolved.agent_id,
        label: resolved.label,
        role: resolved.role,
        prompt: resolved.prompt,
      });
      return { entries: [id], exits: [id] };
    }
    case 'wait': {
      const id = newId('n');
      ctx.nodes.push({
        id,
        type: 'wait',
        label: spec.label ?? 'Wait',
        wait_seconds: spec.seconds,
        wait_for: spec.waitFor,
      });
      return { entries: [id], exits: [id] };
    }
    case 'break': {
      const id = newId('n');
      ctx.nodes.push({ id, type: 'break', label: 'Break' });
      return { entries: [id], exits: [id] };
    }
    case 'parallel': {
      const forkId = newId('n');
      const joinId = newId('n');
      ctx.nodes.push({ id: forkId, type: 'fork', label: 'Fork' });
      ctx.nodes.push({ id: joinId, type: 'join', label: 'Join' });
      const branches =
        spec.branches.length > 0
          ? spec.branches
          : [[{ kind: 'agent' as const, ref: { nameHint: '并行步骤' } }]];
      for (const branch of branches) {
        const part = emit(ctx, { kind: 'seq', steps: branch });
        for (const e of part.entries) link(ctx, forkId, e);
        if (part.exits.length === 0) {
          link(ctx, forkId, joinId);
        } else {
          for (const x of part.exits) link(ctx, x, joinId);
        }
      }
      return { entries: [forkId], exits: [joinId] };
    }
    case 'if': {
      const ifId = newId('n');
      ctx.nodes.push({
        id: ifId,
        type: 'if',
        label: 'If / Else',
        condition: spec.condition,
      });
      const thenPart = emit(ctx, { kind: 'seq', steps: spec.then });
      for (const e of thenPart.entries) link(ctx, ifId, e, 'true');
      const exits = [...thenPart.exits];
      if (spec.else && spec.else.length > 0) {
        const elsePart = emit(ctx, { kind: 'seq', steps: spec.else });
        for (const e of elsePart.entries) link(ctx, ifId, e, 'false');
        exits.push(...elsePart.exits);
      }
      return {
        entries: [ifId],
        exits: exits.length ? exits : [ifId],
      };
    }
    case 'while': {
      const whileId = newId('n');
      ctx.nodes.push({
        id: whileId,
        type: 'while',
        label: 'While',
        condition: spec.condition,
        max_iterations: spec.maxIterations ?? 10,
      });
      const body = emit(ctx, { kind: 'seq', steps: spec.body });
      for (const e of body.entries) link(ctx, whileId, e, 'body');
      for (const x of body.exits) link(ctx, x, whileId);
      // exit edge placeholder: UI may connect later; runtime uses exit branch
      return { entries: [whileId], exits: [whileId] };
    }
    default: {
      const _exhaustive: never = spec;
      return _exhaustive;
    }
  }
}

function emptyDraft(): SquadEditorDraft {
  return {
    name: '',
    leader_agent_id: null,
    target_type: 'issue_and_path',
    issue_id: null,
    working_directory: null,
    pipeline: { nodes: [], edges: [] },
  };
}

export function validatePipeline(pipeline: SquadPipeline): string[] {
  const errors: string[] = [];
  const ids = new Set<string>();
  for (const n of pipeline.nodes) {
    if (!n.id) errors.push('存在无 id 的节点');
    if (ids.has(n.id)) errors.push(`重复节点 id: ${n.id}`);
    ids.add(n.id);
    const t = n.type ?? 'agent';
    if (!NODE_TYPES.includes(t)) {
      errors.push(`未知节点类型: ${String(t)}`);
    }
  }
  for (const e of pipeline.edges) {
    if (!ids.has(e.source) || !ids.has(e.target)) {
      errors.push(`边 ${e.id} 引用了不存在的节点`);
    }
    if (e.branch && !BRANCHES.includes(e.branch)) {
      errors.push(`未知边分支: ${String(e.branch)}`);
    }
  }
  return errors;
}

function sanitizePipeline(
  raw: unknown,
  agents: Agent[],
  warnings: string[]
): SquadPipeline {
  const obj = (raw && typeof raw === 'object' ? raw : {}) as Record<
    string,
    unknown
  >;
  const nodesIn = Array.isArray(obj.nodes) ? obj.nodes : [];
  const edgesIn = Array.isArray(obj.edges) ? obj.edges : [];
  const nodes: SquadPipelineNode[] = [];
  const idMap = new Map<string, string>();

  for (const item of nodesIn) {
    if (!item || typeof item !== 'object') continue;
    const n = item as Record<string, unknown>;
    let type = String(n.type ?? 'agent') as SquadPipelineNodeType;
    if (!NODE_TYPES.includes(type)) {
      warnings.push(`忽略未知类型「${type}」，按 agent 处理`);
      type = 'agent';
    }
    const oldId = typeof n.id === 'string' && n.id ? n.id : newId('n');
    const id = newId('n');
    idMap.set(oldId, id);

    let agent_id: string | undefined;
    if (typeof n.agent_id === 'string' && n.agent_id) {
      agent_id = agents.some((a) => a.id === n.agent_id)
        ? n.agent_id
        : undefined;
      if (!agent_id) warnings.push(`无效 agent_id，已清空`);
    } else if (typeof n.agent_name === 'string' && n.agent_name) {
      const m = matchAgentByName(agents, n.agent_name);
      if (m) agent_id = m.id;
      else warnings.push(`未匹配 Agent「${n.agent_name}」`);
    } else if (typeof n.label === 'string' && type === 'agent') {
      const m = matchAgentByName(agents, n.label);
      if (m) agent_id = m.id;
    }

    const pos =
      n.position && typeof n.position === 'object'
        ? {
            x: Number((n.position as { x?: number }).x) || 0,
            y: Number((n.position as { y?: number }).y) || 0,
          }
        : undefined;

    nodes.push({
      id,
      type,
      agent_id,
      role: typeof n.role === 'string' ? n.role : undefined,
      prompt: typeof n.prompt === 'string' ? n.prompt : undefined,
      label: typeof n.label === 'string' ? n.label : undefined,
      position: pos,
      condition: typeof n.condition === 'string' ? n.condition : undefined,
      max_iterations:
        typeof n.max_iterations === 'number' ? n.max_iterations : undefined,
      wait_seconds:
        typeof n.wait_seconds === 'number' ? n.wait_seconds : undefined,
      wait_for: typeof n.wait_for === 'string' ? n.wait_for : undefined,
      join_count: typeof n.join_count === 'number' ? n.join_count : undefined,
    });
  }

  const edges: SquadPipelineEdge[] = [];
  for (const item of edgesIn) {
    if (!item || typeof item !== 'object') continue;
    const e = item as Record<string, unknown>;
    const source = idMap.get(String(e.source ?? ''));
    const target = idMap.get(String(e.target ?? ''));
    if (!source || !target) continue;
    let branch: SquadPipelineEdgeBranch | undefined;
    if (
      typeof e.branch === 'string' &&
      BRANCHES.includes(e.branch as SquadPipelineEdgeBranch)
    ) {
      branch = e.branch as SquadPipelineEdgeBranch;
    }
    edges.push({
      id: newId('e'),
      source,
      target,
      ...(branch && branch !== 'default' ? { branch } : {}),
    });
  }

  const pipeline: SquadPipeline = { nodes, edges };
  if (obj.loop_config && typeof obj.loop_config === 'object') {
    pipeline.loop_config = obj.loop_config as SquadPipeline['loop_config'];
  }
  assignMissingPositions(pipeline);
  return pipeline;
}

// ── Deterministic NL parsing ─────────────────────────────────────────────────

function stripTargetHints(text: string): {
  rest: string;
  issueHint?: string;
  pathHint?: string;
  nameHint?: string;
  targetType?: SquadTargetType;
} {
  let rest = text;
  let issueHint: string | undefined;
  let pathHint: string | undefined;
  let nameHint: string | undefined;
  let targetType: SquadTargetType | undefined;

  const nameM = rest.match(
    /(?:叫做|命名为|名称[是为]|name\s*[:=])\s*[「『"']?([^」』"'\n，,]+)[」』"']?/i
  );
  if (nameM) {
    nameHint = nameM[1]!.trim();
    rest = rest.replace(nameM[0], ' ');
  }

  const issueM = rest.match(
    /(?:针对|绑定)?\s*(?:Issue|issue|任务)\s*[「『"':：]?\s*([^」』"'\n，,]{1,80})/i
  );
  if (issueM) {
    issueHint = issueM[1]!.trim();
    rest = rest.replace(issueM[0], ' ');
    targetType = 'issue';
  }

  const pathM = rest.match(
    /(?:目录|路径|workdir|cwd|working[_ ]?directory)\s*[「『"':：]?\s*([^\s」』"'\n，,]{1,200})/i
  );
  if (pathM) {
    pathHint = pathM[1]!.trim();
    rest = rest.replace(pathM[0], ' ');
    targetType = targetType === 'issue' ? 'issue_and_path' : 'path';
  }

  // bare absolute / home paths
  const barePath = rest.match(/(?:^|[\s，,])(\/[^\s，,]{2,}|~\/[^\s，,]{2,})/);
  if (!pathHint && barePath) {
    pathHint = barePath[1]!;
    rest = rest.replace(barePath[1]!, ' ');
    targetType = targetType === 'issue' ? 'issue_and_path' : 'path';
  }

  return { rest: rest.trim(), issueHint, pathHint, nameHint, targetType };
}

function parseWaitClause(chunk: string): StepSpec | null {
  const waitSec = chunk.match(
    /(?:wait|等待|睡|sleep)\s*(\d+)\s*(?:秒|s|sec|seconds?)?/i
  );
  if (waitSec) {
    return {
      kind: 'wait',
      seconds: Number(waitSec[1]),
      label: `等待 ${waitSec[1]} 秒`,
    };
  }
  if (/(?:wait|等待)/i.test(chunk) && !/等待[条件|结果]/.test(chunk)) {
    const forM = chunk.match(/(?:wait(?:\s+for)?|等待)\s*(.+)/i);
    return {
      kind: 'wait',
      seconds: 30,
      waitFor: forM?.[1]?.trim() || undefined,
      label: 'Wait',
    };
  }
  return null;
}

function parseAgentClause(chunk: string, agents: Agent[]): StepSpec {
  const wait = parseWaitClause(chunk);
  if (wait) return wait;
  if (/^break$|跳出|中断循环/i.test(chunk.trim())) return { kind: 'break' };

  // 「用小八查代码」 / 「小八负责 review」
  const useM = chunk.match(
    /(?:用|让|由|请)?\s*([^\s，,：:]{1,24}?)\s*(?:来|去|负责|执行)?\s*(.+)?/
  );
  let nameHint = chunk.trim();
  let prompt: string | undefined;
  let role: string | undefined;

  // Prefer longest agent name contained in chunk
  const sorted = [...agents].sort((a, b) => b.name.length - a.name.length);
  for (const a of sorted) {
    if (a.name && chunk.includes(a.name)) {
      nameHint = a.name;
      const after = chunk.split(a.name).slice(1).join(a.name).trim();
      const cleaned = after
        .replace(/^(?:来|去|负责|执行|进行|做)\s*/, '')
        .trim();
      if (cleaned) {
        prompt = cleaned;
        role = cleaned.slice(0, 32);
      }
      break;
    }
  }

  if (nameHint === chunk.trim() && useM) {
    const cand = useM[1]!.trim();
    if (cand.length <= 16) {
      nameHint = cand;
      const rest = (useM[2] ?? '').trim();
      if (rest) {
        prompt = rest;
        role = rest.slice(0, 32);
      }
    }
  }

  // Role-only phrases without agent name
  if (
    !matchAgentByName(agents, nameHint) &&
    /review|测试|汇总|查代码|调研|实现|设计|审查/i.test(chunk)
  ) {
    role = chunk.trim().slice(0, 40);
    prompt = chunk.trim();
    nameHint = role;
  }

  return {
    kind: 'agent',
    ref: { nameHint, role, prompt },
  };
}

function splitParallelBranches(inner: string, agents: Agent[]): StepSpec[][] {
  const parts = inner
    .split(/(?:和|与|、|,|，|以及|同时|还有)/)
    .map((s) => s.trim())
    .filter(Boolean);
  if (parts.length <= 1) {
    return [[parseAgentClause(inner, agents)]];
  }
  return parts.map((p) => [parseAgentClause(p, agents)]);
}

function parseChunk(chunk: string, agents: Agent[]): StepSpec {
  const t = chunk.trim();
  if (!t) return { kind: 'agent', ref: { nameHint: '步骤' } };

  // parallel: 并行 A 和 B / 同时 …
  const parM = t.match(
    /^(?:再)?(?:并行|同时|一起|并发)\s*(?:地)?(?:让|用|做|执行)?\s*(.+)$/i
  );
  if (parM) {
    return {
      kind: 'parallel',
      branches: splitParallelBranches(parM[1]!, agents),
    };
  }
  if (/^(?:fork|join)$/i.test(t)) {
    return t.toLowerCase() === 'fork'
      ? {
          kind: 'parallel',
          branches: [
            [{ kind: 'agent', ref: { nameHint: '分支 A' } }],
            [{ kind: 'agent', ref: { nameHint: '分支 B' } }],
          ],
        }
      : { kind: 'agent', ref: { nameHint: 'Join' } };
  }

  // if … then … else …
  const ifM = t.match(
    /(?:如果|若|if)\s*(.+?)\s*(?:则|就|then)\s*(.+?)(?:\s*(?:否则|不然|else)\s*(.+))?$/i
  );
  if (ifM) {
    return {
      kind: 'if',
      condition: ifM[1]!.trim(),
      then: [parseAgentClause(ifM[2]!.trim(), agents)],
      else: ifM[3] ? [parseAgentClause(ifM[3].trim(), agents)] : undefined,
    };
  }

  // while / 循环
  const whileM = t.match(
    /(?:while|循环(?:执行)?(?:直到)?)\s*(.+?)(?:\s*(?:时|时候)?(?:执行|做)\s*(.+))?$/i
  );
  if (whileM && /while|循环/i.test(t)) {
    return {
      kind: 'while',
      condition: whileM[1]!.trim(),
      body: [parseAgentClause(whileM[2]?.trim() || '循环体', agents)],
      maxIterations: 10,
    };
  }

  return parseAgentClause(t, agents);
}

function splitSequential(text: string): string[] {
  // Keep parallel clauses intact
  const normalized = text
    .replace(/\s*->\s*/g, '，')
    .replace(/\s*→\s*/g, '，')
    .replace(/\s*;\s*/g, '，');

  const parts = normalized
    .split(
      /(?:，|\n)+|(?:^|[\s，])(?:然后|接着|再(?:次)?|最后|其次|之后|and then|then|next|finally)\s+/i
    )
    .map((s) => s.trim())
    .filter((s) => s && !/^(然后|接着|再|最后|其次|之后)$/i.test(s));

  return parts.length ? parts : [text.trim()].filter(Boolean);
}

function matchIssue(issues: Issue[], hint?: string): string | null {
  if (!hint) return null;
  const exact = issues.find((i) => i.title === hint);
  if (exact) return exact.id;
  const nh = normalize(hint);
  const hit = issues.find(
    (i) =>
      normalize(i.title || '').includes(nh) ||
      nh.includes(normalize(i.title || ''))
  );
  return hit?.id ?? null;
}

function buildFromSpec(
  spec: StepSpec,
  agents: Agent[],
  meta: {
    name?: string;
    issueId?: string | null;
    path?: string | null;
    targetType?: SquadTargetType;
  }
): SquadChatGeneration {
  const warnings: string[] = [];
  const ctx: BuildCtx = { agents, warnings, nodes: [], edges: [] };
  emit(ctx, spec);
  const pipeline: SquadPipeline = { nodes: ctx.nodes, edges: ctx.edges };
  assignMissingPositions(pipeline);
  layoutSequential(pipeline.nodes); // ensure if assign missed

  const errs = validatePipeline(pipeline);
  if (errs.length) warnings.push(...errs);

  const draft: SquadEditorDraft = {
    ...emptyDraft(),
    name: meta.name?.trim() || suggestName(pipeline, agents),
    target_type: meta.targetType ?? 'issue_and_path',
    issue_id: meta.issueId ?? null,
    working_directory: meta.path ?? null,
    pipeline,
    leader_agent_id:
      pipeline.nodes.find((n) => n.type === 'agent' && n.agent_id)?.agent_id ??
      null,
  };

  const summary = describePipeline(draft, warnings);
  return { summary, draft, warnings, source: 'template' };
}

function suggestName(pipeline: SquadPipeline, agents: Agent[]): string {
  const labels = pipeline.nodes
    .filter((n) => (n.type ?? 'agent') === 'agent')
    .map((n) => {
      const a = n.agent_id
        ? agents.find((x) => x.id === n.agent_id)?.name
        : undefined;
      return a || n.label || n.role || '步骤';
    })
    .slice(0, 3);
  if (labels.length === 0) return '新流水线';
  return labels.join(' → ');
}

function describePipeline(draft: SquadEditorDraft, warnings: string[]): string {
  const kinds = draft.pipeline.nodes.map((n) => n.type ?? 'agent');
  const agentCount = kinds.filter((k) => k === 'agent').length;
  const hasFork = kinds.includes('fork');
  const parts = [
    `已生成 ${draft.pipeline.nodes.length} 个节点、${draft.pipeline.edges.length} 条边`,
    agentCount ? `${agentCount} 个 Agent 步骤` : null,
    hasFork ? '含并行 Fork/Join' : null,
    draft.name ? `建议名称「${draft.name}」` : null,
  ].filter(Boolean);
  let s = parts.join(' · ') + '。可在画布上微调后保存。';
  if (warnings.length) {
    s += `\n注意：${warnings.slice(0, 4).join('；')}`;
  }
  return s;
}

/** Parse natural language into a Squad draft (deterministic). */
export function generateSquadFromTemplate(params: {
  message: string;
  agents: Agent[];
  issues: Issue[];
  current?: SquadEditorDraft | null;
}): SquadChatGeneration {
  const { message, agents, issues, current } = params;
  const trimmed = message.trim();
  if (!trimmed) {
    return {
      summary:
        '请描述流水线，例如：「先用小八查代码，再并行 review，最后汇总」。',
      draft: current ?? emptyDraft(),
      warnings: [],
      source: 'template',
    };
  }

  // Patch intents against current pipeline
  if (current && current.pipeline.nodes.length > 0) {
    const patch = tryPatchPipeline(trimmed, current, agents);
    if (patch) return patch;
  }

  const { rest, issueHint, pathHint, nameHint, targetType } =
    stripTargetHints(trimmed);

  const chunks = splitSequential(rest);
  const steps: StepSpec[] = chunks.map((c) => parseChunk(c, agents));

  // Collapse lone empty
  const spec: StepSpec =
    steps.length === 1 ? steps[0]! : { kind: 'seq', steps };

  const issueId = matchIssue(issues, issueHint);
  const warnings: string[] = [];
  if (issueHint && !issueId) {
    warnings.push(`未找到 Issue「${issueHint}」，请在编辑器中手动选择`);
  }

  const result = buildFromSpec(spec, agents, {
    name: nameHint || current?.name,
    issueId: issueId ?? current?.issue_id ?? null,
    path: pathHint ?? current?.working_directory ?? null,
    targetType:
      targetType ??
      current?.target_type ??
      (pathHint || issueId ? undefined : 'issue_and_path'),
  });
  result.warnings = [...warnings, ...result.warnings];
  result.summary = describePipeline(result.draft, result.warnings);
  return result;
}

function sinks(pipeline: SquadPipeline): string[] {
  const hasOut = new Set(pipeline.edges.map((e) => e.source));
  const sinksIds = pipeline.nodes
    .map((n) => n.id)
    .filter((id) => !hasOut.has(id));
  return sinksIds.length ? sinksIds : pipeline.nodes.slice(-1).map((n) => n.id);
}

function tryPatchPipeline(
  message: string,
  current: SquadEditorDraft,
  agents: Agent[]
): SquadChatGeneration | null {
  const m = message.trim();

  // 再加一个 wait / 加 wait 30秒
  const addWait = m.match(
    /(?:再)?(?:加|添加|增加|插入)\s*(?:一个)?\s*(?:wait|等待)(?:\s*(\d+)\s*(?:秒|s)?)?/i
  );
  if (addWait) {
    const seconds = addWait[1] ? Number(addWait[1]) : 30;
    const id = newId('n');
    const node: SquadPipelineNode = {
      id,
      type: 'wait',
      label: `等待 ${seconds} 秒`,
      wait_seconds: seconds,
    };
    const nodes = [...current.pipeline.nodes, node];
    const edges = [...current.pipeline.edges];
    for (const s of sinks(current.pipeline)) {
      edges.push({ id: newId('e'), source: s, target: id });
    }
    const pipeline = { ...current.pipeline, nodes, edges };
    assignMissingPositions(pipeline);
    const draft = { ...current, pipeline };
    return {
      summary: `已追加 Wait（${seconds} 秒）。`,
      draft,
      warnings: [],
      source: 'patch',
    };
  }

  // 再加一步 / 加上 XXX
  const addStep = m.match(
    /(?:再)?(?:加|添加|增加)\s*(?:一个|一步)?\s*(?:步骤|agent)?\s*[：:]?\s*(.+)$/i
  );
  if (addStep && !/wait|等待|fork|join|并行/i.test(addStep[1] ?? '')) {
    const step = parseAgentClause(addStep[1]!.trim(), agents);
    const warnings: string[] = [];
    const ctx: BuildCtx = {
      agents,
      warnings,
      nodes: [...current.pipeline.nodes],
      edges: [...current.pipeline.edges],
    };
    const part = emit(ctx, step);
    for (const s of sinks(current.pipeline)) {
      for (const e of part.entries) link(ctx, s, e);
    }
    const pipeline = { nodes: ctx.nodes, edges: ctx.edges };
    assignMissingPositions(pipeline);
    const draft = { ...current, pipeline };
    return {
      summary: `已追加步骤。${warnings.length ? warnings.join('；') : ''}`,
      draft,
      warnings,
      source: 'patch',
    };
  }

  // 再并行 XXX
  const addPar = m.match(
    /(?:再)?(?:加|添加)?\s*(?:一个)?\s*(?:并行|同时)\s*(.+)$/i
  );
  if (addPar) {
    const warnings: string[] = [];
    const ctx: BuildCtx = {
      agents,
      warnings,
      nodes: [...current.pipeline.nodes],
      edges: [...current.pipeline.edges],
    };
    const part = emit(ctx, {
      kind: 'parallel',
      branches: splitParallelBranches(addPar[1]!, agents),
    });
    for (const s of sinks(current.pipeline)) {
      for (const e of part.entries) link(ctx, s, e);
    }
    const pipeline = { nodes: ctx.nodes, edges: ctx.edges };
    assignMissingPositions(pipeline);
    return {
      summary: `已追加并行 Fork/Join 段。`,
      draft: { ...current, pipeline },
      warnings,
      source: 'patch',
    };
  }

  // 改名
  const rename = m.match(
    /(?:改名|重命名|叫做|命名为)\s*[「『"']?([^」』"']+)[」』"']?/
  );
  if (rename) {
    return {
      summary: `已将名称改为「${rename[1]!.trim()}」。`,
      draft: { ...current, name: rename[1]!.trim() },
      warnings: [],
      source: 'patch',
    };
  }

  return null;
}

// ── Optional LLM via sidecar ─────────────────────────────────────────────────

function buildLlmPrompt(
  message: string,
  agents: Agent[],
  current?: SquadEditorDraft | null
): string {
  const agentList = agents.map((a) => `- ${a.name} (id=${a.id})`).join('\n');
  const currentJson = current
    ? JSON.stringify(
        {
          name: current.name,
          target_type: current.target_type,
          issue_id: current.issue_id,
          working_directory: current.working_directory,
          pipeline: current.pipeline,
        },
        null,
        2
      )
    : 'null';

  return `你是 Squad 流水线生成器。根据用户描述，输出【仅】一个 JSON 对象（不要 markdown 代码块以外的解释），schema：
{
  "name": string,
  "summary": string,  // 中文简述
  "target_type": "issue" | "path" | "issue_and_path",
  "issue_id": string | null,
  "working_directory": string | null,
  "pipeline": {
    "nodes": [{
      "id": string,
      "type": "agent"|"if"|"while"|"break"|"wait"|"fork"|"join",
      "agent_id"?: string,
      "agent_name"?: string,
      "label"?: string,
      "role"?: string,
      "prompt"?: string,
      "condition"?: string,
      "max_iterations"?: number,
      "wait_seconds"?: number,
      "wait_for"?: string,
      "join_count"?: number,
      "position"?: {"x": number, "y": number}
    }],
    "edges": [{
      "id": string,
      "source": string,
      "target": string,
      "branch"?: "default"|"true"|"false"|"body"|"exit"|"error"
    }]
  }
}

规则：
- 并行用 fork → 多分支 → join
- agent 节点尽量用下方 agent_id；不确定时用 agent_name，可空着 agent_id
- 若用户在改已有流水线，基于 current 做增量修改
- position 可省略（前端会自动布局）

可用 Agent：
${agentList || '(无)'}

current:
${currentJson}

用户需求：
${message}`;
}

function extractJsonObject(text: string): unknown | null {
  const fenced = text.match(/```(?:json)?\s*([\s\S]*?)```/i);
  const raw = fenced ? fenced[1]!.trim() : text.trim();
  const start = raw.indexOf('{');
  const end = raw.lastIndexOf('}');
  if (start < 0 || end <= start) return null;
  try {
    return JSON.parse(raw.slice(start, end + 1));
  } catch {
    return null;
  }
}

/** First project agent that has a saved LLM API key (checks up to 12). */
export async function findAgentWithApiKey(
  agents: Agent[]
): Promise<{ agentId: string; name: string } | null> {
  for (const a of agents.slice(0, 12)) {
    try {
      const s = await boardAgentsApi.getLlmSettings(a.id);
      if (s.has_api_key) return { agentId: a.id, name: a.name };
    } catch {
      // ignore
    }
  }
  return null;
}

function withLlmFallback(
  template: SquadChatGeneration,
  reason: string
): SquadChatGeneration {
  const note = `Agent 生成失败：${reason}。已回退模板解析。`;
  return {
    ...template,
    summary: `${note}\n${template.summary}`,
    warnings: [note, ...template.warnings],
    source: 'template',
    llmError: reason,
  };
}

export async function generateSquadFromChat(params: {
  message: string;
  agents: Agent[];
  issues: Issue[];
  projectId: string;
  current?: SquadEditorDraft | null;
  /**
   * Prefer Agent/LLM generation.
   * Default: true when any project agent has an API key; otherwise false.
   */
  preferLlm?: boolean;
  onStatus?: (status: SquadChatGenStatus) => void;
}): Promise<SquadChatGeneration> {
  const { message, agents, issues, projectId, current, onStatus } = params;

  // Fast path: patches always stay local / offline
  const template = generateSquadFromTemplate({
    message,
    agents,
    issues,
    current,
  });

  if (template.source === 'patch') {
    onStatus?.('template');
    return template;
  }

  onStatus?.('checking');
  const keyed = await findAgentWithApiKey(agents);
  const preferLlm = params.preferLlm ?? Boolean(keyed);

  if (!preferLlm) {
    onStatus?.('template');
    return template;
  }

  if (!keyed) {
    onStatus?.('template');
    return withLlmFallback(template, '没有配置 API Key 的 Agent');
  }

  onStatus?.('llm');

  try {
    const token = await getAuthRuntime().getToken();
    if (!token) {
      return withLlmFallback(template, '未登录或缺少 access token');
    }

    const session = await boardAgentsApi.createSession({
      project_id: projectId,
      agent_id: keyed.agentId,
      title: 'Squad 对话创建',
    });

    const { reply } = await boardAgentsApi.chatStream({
      project_id: projectId,
      session_id: session.id,
      agent_id: keyed.agentId,
      message: buildLlmPrompt(message, agents, current),
      token,
    });

    if (!reply?.trim()) {
      return withLlmFallback(template, 'Agent 返回为空');
    }

    const parsed = extractJsonObject(reply);
    if (!parsed || typeof parsed !== 'object') {
      return withLlmFallback(template, 'Agent 回复无法解析为 JSON pipeline');
    }

    const obj = parsed as Record<string, unknown>;
    const warnings: string[] = [];
    const pipeline = sanitizePipeline(obj.pipeline, agents, warnings);
    const errs = validatePipeline(pipeline);
    if (errs.length || pipeline.nodes.length === 0) {
      return withLlmFallback(
        template,
        errs.length
          ? `pipeline 校验失败：${errs.slice(0, 2).join('；')}`
          : 'pipeline 节点为空'
      );
    }

    const targetRaw = String(
      obj.target_type ?? current?.target_type ?? 'issue_and_path'
    );
    const target_type: SquadTargetType =
      targetRaw === 'issue' ||
      targetRaw === 'path' ||
      targetRaw === 'issue_and_path'
        ? targetRaw
        : 'issue_and_path';

    const draft: SquadEditorDraft = {
      name:
        (typeof obj.name === 'string' && obj.name.trim()) ||
        current?.name ||
        suggestName(pipeline, agents),
      leader_agent_id:
        pipeline.nodes.find((n) => n.type === 'agent' && n.agent_id)
          ?.agent_id ??
        current?.leader_agent_id ??
        null,
      target_type,
      issue_id:
        typeof obj.issue_id === 'string'
          ? obj.issue_id
          : (current?.issue_id ?? null),
      working_directory:
        typeof obj.working_directory === 'string'
          ? obj.working_directory
          : (current?.working_directory ?? null),
      pipeline,
    };

    const summary =
      (typeof obj.summary === 'string' && obj.summary.trim()) ||
      describePipeline(draft, warnings);

    return {
      summary,
      draft,
      warnings,
      source: 'llm',
    };
  } catch (e) {
    const reason = e instanceof Error ? e.message : String(e);
    return withLlmFallback(template, reason);
  }
}
