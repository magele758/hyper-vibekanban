/**
 * Built-in Feature Babysitter pipeline template.
 * Implement → Verify → while(needs_work) { Fix → Verify } → Rebase → Merge gate
 */
import type {
  Agent,
  SquadPipeline,
  SquadPipelineEdge,
  SquadPipelineNode,
} from 'shared/remote-types';

const COL = 240;
const ROW = 120;

export const BABYSITTER_VERIFY_PROMPT = `你是代码审查 + 验收助手。对照 Issue 验收标准检查当前工作区改动：
1. 功能是否完整、边界是否覆盖
2. 是否有合理测试（没有则指出缺口或补上）
3. 是否有明显 bug / 回归风险

必须在最终评论（或回复末尾）单独一行写出以下之一：
BABYSITTER_VERDICT: READY
或
BABYSITTER_VERDICT: NEEDS_WORK: <具体清单，分号分隔>

不要省略 verdict 行。`;

export const BABYSITTER_IMPLEMENT_PROMPT = `实现 Issue 描述的功能。写完后自行跑相关检查/测试；不要声称完成但未验证。`;

export const BABYSITTER_FIX_PROMPT = `根据上一步 Verify 的 NEEDS_WORK 清单修复问题，并再次自测。修复后不要输出 READY（下一步会再 Verify）。`;

function id(prefix: string): string {
  return `${prefix}_${Math.random().toString(36).slice(2, 10)}`;
}

export function buildFeatureBabysitterPipeline(opts: {
  implementAgentId?: string;
  verifyAgentId?: string;
  maxFixRounds?: number;
}): SquadPipeline {
  const maxFix = Math.max(1, Math.min(opts.maxFixRounds ?? 3, 10));
  const implement: SquadPipelineNode = {
    id: id('impl'),
    type: 'agent',
    label: 'Implement',
    role: 'implementer',
    prompt: BABYSITTER_IMPLEMENT_PROMPT,
    agent_id: opts.implementAgentId,
    position: { x: 24, y: 40 },
  };
  const verify1: SquadPipelineNode = {
    id: id('verify'),
    type: 'agent',
    label: 'Verify',
    role: 'reviewer',
    prompt: BABYSITTER_VERIFY_PROMPT,
    agent_id: opts.verifyAgentId ?? opts.implementAgentId,
    position: { x: 24 + COL, y: 40 },
  };
  const loop: SquadPipelineNode = {
    id: id('while'),
    type: 'while',
    label: 'Until READY',
    condition: 'verdict:needs_work',
    max_iterations: maxFix,
    position: { x: 24 + COL * 2, y: 40 },
  };
  const fix: SquadPipelineNode = {
    id: id('fix'),
    type: 'agent',
    label: 'Fix',
    role: 'implementer',
    prompt: BABYSITTER_FIX_PROMPT,
    agent_id: opts.implementAgentId,
    position: { x: 24 + COL * 2, y: 40 + ROW },
  };
  const verify2: SquadPipelineNode = {
    id: id('reverify'),
    type: 'agent',
    label: 'Re-verify',
    role: 'reviewer',
    prompt: BABYSITTER_VERIFY_PROMPT,
    agent_id: opts.verifyAgentId ?? opts.implementAgentId,
    position: { x: 24 + COL * 3, y: 40 + ROW },
  };
  const rebase: SquadPipelineNode = {
    id: id('rebase'),
    type: 'rebase',
    label: 'Rebase',
    position: { x: 24 + COL * 3, y: 40 },
  };
  const gate: SquadPipelineNode = {
    id: id('gate'),
    type: 'human_gate',
    label: '合并确认',
    gate_kind: 'merge_approval',
    prompt: 'Feature 已完成验收与 rebase，是否合并到目标分支？',
    position: { x: 24 + COL * 4, y: 40 },
  };

  const nodes = [implement, verify1, loop, fix, verify2, rebase, gate];
  const edges: SquadPipelineEdge[] = [
    { id: id('e'), source: implement.id, target: verify1.id },
    { id: id('e'), source: verify1.id, target: loop.id },
    {
      id: id('e'),
      source: loop.id,
      target: fix.id,
      branch: 'body',
    },
    { id: id('e'), source: fix.id, target: verify2.id },
    {
      id: id('e'),
      source: loop.id,
      target: rebase.id,
      branch: 'exit',
    },
    { id: id('e'), source: rebase.id, target: gate.id },
  ];

  return { nodes, edges };
}

/** Prefer distinct verify agent if name hints reviewer; else same as leader. */
export function pickBabysitterAgents(agents: Agent[]): {
  implementAgentId?: string;
  verifyAgentId?: string;
} {
  if (agents.length === 0) return {};
  const reviewer = agents.find((a) =>
    /review|验|审|qa|test/i.test(a.name)
  );
  const implement = agents.find((a) =>
    /impl|dev|code|写|实现/i.test(a.name)
  );
  const leader = implement ?? agents[0];
  return {
    implementAgentId: leader.id,
    verifyAgentId: reviewer?.id ?? leader.id,
  };
}
